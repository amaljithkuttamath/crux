pub mod analysis;
pub mod cursor;

use crate::parser::{Source, UsageRecord};
use crate::parser::conversation::SessionMeta;
use crate::pricing;
use chrono::{Duration, NaiveDate, Utc};
use std::collections::{HashMap, HashSet};

pub use analysis::{SessionAnalysis, SessionTimeline};
pub use cursor::{CursorModelStat, CursorOverviewStats};

#[derive(Clone, Default)]
pub struct Store {
    records: Vec<UsageRecord>,
    session_metas: Vec<SessionMeta>,
}

#[derive(Debug, Default)]
pub struct Aggregation {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub session_count: usize,
    pub record_count: usize,
    pub cost: f64,
}

#[derive(Debug)]
pub struct ProjectSummary {
    pub name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub session_count: usize,
    pub last_used: chrono::DateTime<Utc>,
    pub cost: f64,
}

#[derive(Debug)]
pub struct DaySummary {
    pub date: NaiveDate,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub session_count: usize,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub struct ModelSummary {
    pub name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub record_count: usize,
    pub cost: f64,
}

impl Aggregation {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }
}

impl Store {

    pub fn add(&mut self, record: UsageRecord) {
        self.records.push(record);
    }

    pub fn add_session_meta(&mut self, meta: SessionMeta) {
        self.session_metas.push(meta);
    }

    /// Sessions sorted by start time (most recent first)
    pub fn sessions_by_time(&self) -> Vec<&SessionMeta> {
        let mut sessions: Vec<&SessionMeta> = self.session_metas.iter().collect();
        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        sessions
    }

    /// Sessions filtered by source, sorted by start time (most recent first)
    pub fn sessions_by_source(&self, source: Source) -> Vec<&SessionMeta> {
        let mut sessions: Vec<&SessionMeta> = self.session_metas.iter()
            .filter(|s| s.source == source)
            .collect();
        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        sessions
    }

    /// Sessions active in the last N hours, with full analysis
    pub fn active_sessions(&self, hours: i64) -> Vec<(&SessionMeta, SessionAnalysis)> {
        let cutoff = Utc::now() - Duration::hours(hours);
        let mut active: Vec<(&SessionMeta, SessionAnalysis)> = self.session_metas.iter()
            .filter(|s| s.end_time >= cutoff && s.user_count > 0)
            .filter_map(|s| {
                let a = self.analyze_session(&s.session_id)?;
                Some((s, a))
            })
            .collect();
        active.sort_by(|a, b| b.0.end_time.cmp(&a.0.end_time));
        active
    }

    /// Search sessions by keyword in first_message
    pub fn search_sessions(&self, query: &str) -> Vec<&SessionMeta> {
        let q = query.to_lowercase();
        let mut results: Vec<&SessionMeta> = self.session_metas.iter()
            .filter(|s| s.first_message.to_lowercase().contains(&q)
                || s.project.to_lowercase().contains(&q))
            .collect();
        results.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        results
    }

    pub fn session_cost(&self, session_id: &str) -> f64 {
        self.records.iter()
            .filter(|r| r.session_id == session_id)
            .map(|r| pricing::estimate_cost(&r.model, r.input_tokens, r.output_tokens,
                r.cache_creation_tokens, r.cache_read_tokens))
            .sum()
    }

    fn aggregate_records(&self, filter: impl Fn(&UsageRecord) -> bool) -> Aggregation {
        let mut agg = Aggregation::default();
        let mut sessions = HashSet::new();
        for r in &self.records {
            if filter(r) {
                agg.input_tokens += r.input_tokens;
                agg.output_tokens += r.output_tokens;
                agg.cache_creation_tokens += r.cache_creation_tokens;
                agg.cache_read_tokens += r.cache_read_tokens;
                agg.record_count += 1;
                agg.cost += pricing::estimate_cost(
                    &r.model, r.input_tokens, r.output_tokens,
                    r.cache_creation_tokens, r.cache_read_tokens,
                );
                sessions.insert(&r.session_id);
            }
        }
        agg.session_count = sessions.len();
        agg
    }

    pub fn today(&self) -> Aggregation {
        let today = Utc::now().date_naive();
        self.aggregate_records(|r| r.timestamp.date_naive() == today)
    }

    pub fn yesterday(&self) -> Aggregation {
        let yesterday = (Utc::now() - Duration::days(1)).date_naive();
        self.aggregate_records(|r| r.timestamp.date_naive() == yesterday)
    }

    pub fn this_week(&self) -> Aggregation {
        let cutoff = Utc::now() - Duration::days(7);
        self.aggregate_records(|r| r.timestamp >= cutoff)
    }

    pub fn all_time(&self) -> Aggregation {
        self.aggregate_records(|_| true)
    }

    pub fn by_source(&self) -> HashMap<Source, Aggregation> {
        let mut map: HashMap<Source, Aggregation> = HashMap::new();
        for r in &self.records {
            let agg = map.entry(r.source).or_default();
            agg.input_tokens += r.input_tokens;
            agg.output_tokens += r.output_tokens;
            agg.cache_creation_tokens += r.cache_creation_tokens;
            agg.cache_read_tokens += r.cache_read_tokens;
            agg.record_count += 1;
            agg.cost += pricing::estimate_cost(
                &r.model, r.input_tokens, r.output_tokens,
                r.cache_creation_tokens, r.cache_read_tokens,
            );
        }
        let mut session_sets: HashMap<Source, HashSet<&String>> = HashMap::new();
        for r in &self.records {
            session_sets.entry(r.source).or_default().insert(&r.session_id);
        }
        for (src, sessions) in session_sets {
            if let Some(agg) = map.get_mut(&src) {
                agg.session_count = sessions.len();
            }
        }
        map
    }

    pub fn by_model(&self) -> Vec<ModelSummary> {
        let mut map: HashMap<String, ModelSummary> = HashMap::new();
        for r in &self.records {
            let key = simplify_model(&r.model);
            let entry = map.entry(key.clone()).or_insert(ModelSummary {
                name: key,
                input_tokens: 0,
                output_tokens: 0,
                record_count: 0,
                cost: 0.0,
            });
            entry.input_tokens += r.input_tokens;
            entry.output_tokens += r.output_tokens;
            entry.record_count += 1;
            entry.cost += pricing::estimate_cost(
                &r.model, r.input_tokens, r.output_tokens,
                r.cache_creation_tokens, r.cache_read_tokens,
            );
        }
        let mut models: Vec<ModelSummary> = map.into_values().collect();
        models.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));
        models
    }

    pub fn by_project(&self) -> Vec<ProjectSummary> {
        let mut map: HashMap<String, ProjectSummary> = HashMap::new();
        let mut session_sets: HashMap<String, HashSet<String>> = HashMap::new();

        for r in &self.records {
            let entry = map.entry(r.project.clone()).or_insert(ProjectSummary {
                name: r.project.clone(),
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                session_count: 0,
                last_used: r.timestamp,
                cost: 0.0,
            });
            entry.input_tokens += r.input_tokens;
            entry.output_tokens += r.output_tokens;
            entry.cache_creation_tokens += r.cache_creation_tokens;
            entry.cache_read_tokens += r.cache_read_tokens;
            entry.cost += pricing::estimate_cost(
                &r.model, r.input_tokens, r.output_tokens,
                r.cache_creation_tokens, r.cache_read_tokens,
            );
            if r.timestamp > entry.last_used {
                entry.last_used = r.timestamp;
            }
            session_sets
                .entry(r.project.clone())
                .or_default()
                .insert(r.session_id.clone());
        }

        let mut projects: Vec<ProjectSummary> = map.into_values().collect();
        for p in &mut projects {
            p.session_count = session_sets.get(&p.name).map(|s| s.len()).unwrap_or(0);
        }
        projects.sort_by(|a, b| b.last_used.cmp(&a.last_used));
        projects
    }

    pub fn by_day(&self, days: usize) -> Vec<DaySummary> {
        let mut map: HashMap<NaiveDate, DaySummary> = HashMap::new();
        let mut session_sets: HashMap<NaiveDate, HashSet<String>> = HashMap::new();

        for r in &self.records {
            let date = r.timestamp.date_naive();
            let entry = map.entry(date).or_insert(DaySummary {
                date,
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                session_count: 0,
                cost: 0.0,
            });
            entry.input_tokens += r.input_tokens;
            entry.output_tokens += r.output_tokens;
            entry.cache_creation_tokens += r.cache_creation_tokens;
            entry.cache_read_tokens += r.cache_read_tokens;
            entry.cost += pricing::estimate_cost(
                &r.model, r.input_tokens, r.output_tokens,
                r.cache_creation_tokens, r.cache_read_tokens,
            );
            session_sets
                .entry(date)
                .or_default()
                .insert(r.session_id.clone());
        }

        let mut days_vec: Vec<DaySummary> = map.into_values().collect();
        for d in &mut days_vec {
            d.session_count = session_sets.get(&d.date).map(|s| s.len()).unwrap_or(0);
        }
        days_vec.sort_by(|a, b| b.date.cmp(&a.date));
        days_vec.truncate(days);
        days_vec
    }

    pub fn analyze_session(&self, session_id: &str) -> Option<SessionAnalysis> {
        analysis::analyze_session(&self.records, session_id)
    }

    pub fn session_timeline(&self, session_id: &str) -> Option<SessionTimeline> {
        analysis::session_timeline(&self.records, session_id)
    }

    pub fn session_tokens(&self, session_id: &str) -> (u64, u64) {
        let input: u64 = self.records.iter()
            .filter(|r| r.session_id == session_id)
            .map(|r| r.input_tokens)
            .sum();
        let output: u64 = self.records.iter()
            .filter(|r| r.session_id == session_id)
            .map(|r| r.output_tokens)
            .sum();
        (input, output)
    }

    pub fn session_model(&self, session_id: &str) -> String {
        self.records.iter()
            .find(|r| r.session_id == session_id)
            .map(|r| r.model.clone())
            .unwrap_or_default()
    }

    pub fn most_recent_session_id(&self) -> Option<String> {
        self.records
            .iter()
            .max_by_key(|r| r.timestamp)
            .map(|r| r.session_id.clone())
    }

    pub fn avg_session_cost_historical(&self) -> f64 {
        let all = self.all_time();
        if all.session_count == 0 { return 0.0; }
        all.cost / all.session_count as f64
    }

    pub fn streak_days(&self) -> usize {
        let mut dates: HashSet<NaiveDate> = HashSet::new();
        for r in &self.records {
            dates.insert(r.timestamp.date_naive());
        }
        let today = Utc::now().date_naive();
        let mut streak = 0usize;
        let start = if dates.contains(&today) { today } else { today - Duration::days(1) };
        if !dates.contains(&start) { return 0; }
        let mut day = start;
        while dates.contains(&day) {
            streak += 1;
            day -= Duration::days(1);
        }
        streak
    }

    pub fn sessions_per_day(&self, days: usize) -> Vec<f64> {
        let day_data = self.by_day(days);
        let today = Utc::now().date_naive();
        let mut result = vec![0.0; days];
        for d in &day_data {
            let age = (today - d.date).num_days() as usize;
            if age < days {
                result[days - 1 - age] = d.session_count as f64;
            }
        }
        result
    }

    // Cursor delegations
    pub fn cursor_sessions(&self) -> Vec<&SessionMeta> {
        cursor::cursor_sessions(&self.session_metas)
    }

    pub fn cursor_model_stats(&self) -> Vec<CursorModelStat> {
        cursor::cursor_model_stats(&self.session_metas, &self.records)
    }

    pub fn cursor_overview_stats(&self) -> CursorOverviewStats {
        cursor::cursor_overview_stats(&self.session_metas, &self.records)
    }

    /// Today's aggregation filtered by source
    pub fn today_by_source(&self, source: Source) -> Aggregation {
        let today = Utc::now().date_naive();
        self.aggregate_records(|r| r.timestamp.date_naive() == today && r.source == source)
    }

    /// Count sessions by source for today
    pub fn today_sessions_by_source(&self, source: Source) -> Vec<&SessionMeta> {
        let today = Utc::now().date_naive();
        let mut sessions: Vec<&SessionMeta> = self.session_metas.iter()
            .filter(|s| s.source == source && s.start_time.date_naive() == today)
            .collect();
        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        sessions
    }
}

pub fn simplify_model(model: &str) -> String {
    if model.contains("opus") {
        "opus".to_string()
    } else if model.contains("haiku") {
        "haiku".to_string()
    } else if model.contains("sonnet") {
        "sonnet".to_string()
    } else if model.is_empty() {
        "unknown".to_string()
    } else {
        model.to_string()
    }
}
