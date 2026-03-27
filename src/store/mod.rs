pub mod analysis;
pub mod cursor;

use crate::parser::{Source, UsageRecord};
use crate::parser::conversation::SessionMeta;
use crate::pricing;
use chrono::{Datelike, Duration, Local, NaiveDate, Utc};
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
        let today = Local::now().date_naive();
        self.aggregate_records(|r| r.timestamp.with_timezone(&Local).date_naive() == today)
    }

    /// Cost per hour based on the last hour of activity.
    /// Returns 0.0 if no activity in the last hour.
    #[allow(dead_code)]
    pub fn burn_rate(&self) -> f64 {
        let one_hour_ago = Utc::now() - Duration::hours(1);
        let cost: f64 = self.records.iter()
            .filter(|r| r.timestamp >= one_hour_ago)
            .map(|r| pricing::estimate_cost(
                &r.model, r.input_tokens, r.output_tokens,
                r.cache_creation_tokens, r.cache_read_tokens,
            ))
            .sum();
        cost
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
        let ceiling = self.session_meta(session_id)
            .and_then(|m| m.context_token_limit);
        analysis::session_timeline(&self.records, session_id, ceiling)
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

    #[allow(dead_code)]
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

    pub fn session_meta(&self, session_id: &str) -> Option<&SessionMeta> {
        self.session_metas.iter().find(|s| s.session_id == session_id)
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

    /// Longest streak ever (consecutive days with activity)
    pub fn longest_streak(&self) -> usize {
        let mut dates: Vec<NaiveDate> = self.records.iter()
            .map(|r| r.timestamp.date_naive())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        dates.sort();
        if dates.is_empty() { return 0; }
        let mut best = 1usize;
        let mut current = 1usize;
        for i in 1..dates.len() {
            if dates[i] - dates[i - 1] == Duration::days(1) {
                current += 1;
                best = best.max(current);
            } else {
                current = 1;
            }
        }
        best
    }

    /// Number of unique active days
    pub fn active_days(&self) -> usize {
        self.records.iter()
            .map(|r| r.timestamp.date_naive())
            .collect::<HashSet<_>>()
            .len()
    }

    /// Total tokens across all records
    pub fn total_tokens(&self) -> u64 {
        self.records.iter()
            .map(|r| r.input_tokens + r.output_tokens + r.cache_creation_tokens + r.cache_read_tokens)
            .sum()
    }

    /// Longest session by duration, returns (session_id, duration_minutes)
    pub fn longest_session(&self) -> Option<(String, f64)> {
        self.session_metas.iter()
            .filter(|s| s.user_count > 0)
            .map(|s| {
                let mins = (s.end_time - s.start_time).num_seconds() as f64 / 60.0;
                (s.session_id.clone(), mins)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Most active day by session count, returns (date, session_count)
    pub fn most_active_day(&self) -> Option<(NaiveDate, usize)> {
        let days = self.by_day(365);
        days.into_iter()
            .max_by_key(|d| d.session_count)
            .map(|d| (d.date, d.session_count))
    }

    /// Activity heatmap: 13 weeks x 7 days grid of session counts.
    /// Returns vec of 91 entries (week 0 = oldest), each is session count for that day.
    /// Also returns the month labels and their column positions.
    pub fn activity_heatmap(&self) -> (Vec<u32>, Vec<(String, usize)>) {
        let today = Local::now().date_naive();
        let today_weekday = today.weekday().num_days_from_monday() as i64; // Mon=0
        let grid_end = today;
        let grid_start = grid_end - Duration::days(90 + today_weekday); // 13 full weeks

        let mut date_counts: HashMap<NaiveDate, u32> = HashMap::new();
        for r in &self.records {
            let d = r.timestamp.with_timezone(&Local).date_naive();
            if d >= grid_start && d <= grid_end {
                *date_counts.entry(d).or_default() += 1;
            }
        }

        let total_days = (grid_end - grid_start).num_days() + 1;
        let mut grid = Vec::with_capacity(total_days as usize);
        let mut month_labels: Vec<(String, usize)> = Vec::new();
        let mut last_month = 0u32;

        for i in 0..total_days {
            let d = grid_start + Duration::days(i);
            let count = date_counts.get(&d).copied().unwrap_or(0);
            grid.push(count);

            let week = i as usize / 7;
            let m = d.month();
            if m != last_month {
                let label = d.format("%b").to_string();
                month_labels.push((label, week));
                last_month = m;
            }
        }

        (grid, month_labels)
    }

    /// Favorite model (most used by cost)
    pub fn favorite_model(&self) -> Option<String> {
        self.by_model().first().map(|m| m.name.clone())
    }

    /// Peak hour of day (0-23) by session count across all data
    pub fn peak_hour(&self) -> Option<u32> {
        use chrono::Timelike;
        let mut hours = [0u32; 24];
        for r in &self.records {
            let h = r.timestamp.with_timezone(&Local).hour() as usize;
            if h < 24 { hours[h] += 1; }
        }
        let max = hours.iter().max().copied().unwrap_or(0);
        if max == 0 { return None; }
        hours.iter().position(|&c| c == max).map(|h| h as u32)
    }

    /// Hourly distribution across all data (0-23), returns session counts per hour
    pub fn hourly_distribution(&self) -> [u32; 24] {
        use chrono::Timelike;
        let mut hours = [0u32; 24];
        let mut seen: HashMap<(u32, String), bool> = HashMap::new();
        for r in &self.records {
            let h = r.timestamp.with_timezone(&Local).hour();
            let key = (h, r.session_id.clone());
            if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(key) {
                e.insert(true);
                hours[h as usize] += 1;
            }
        }
        hours
    }

    /// Day-of-week distribution (Mon=0..Sun=6), returns session counts
    pub fn weekday_distribution(&self) -> [u32; 7] {
        let mut days = [0u32; 7];
        let mut seen: HashSet<(u32, String)> = HashSet::new();
        for r in &self.records {
            let wd = r.timestamp.with_timezone(&Local).date_naive().weekday().num_days_from_monday();
            let key = (wd, r.session_id.clone());
            if seen.insert(key) {
                days[wd as usize] += 1;
            }
        }
        days
    }

    /// Night owl ratio: percentage of sessions between 10pm and 5am
    pub fn night_owl_ratio(&self) -> f64 {
        use chrono::Timelike;
        let mut night_sessions: HashSet<String> = HashSet::new();
        let mut all_sessions: HashSet<String> = HashSet::new();
        for r in &self.records {
            all_sessions.insert(r.session_id.clone());
            let h = r.timestamp.with_timezone(&Local).hour();
            if !(5..22).contains(&h) {
                night_sessions.insert(r.session_id.clone());
            }
        }
        if all_sessions.is_empty() { return 0.0; }
        night_sessions.len() as f64 / all_sessions.len() as f64 * 100.0
    }

    /// Grade distribution across all sessions: returns (A, B, C, D, F) counts
    pub fn grade_distribution(&self) -> [usize; 5] {
        let mut grades = [0usize; 5];
        for meta in &self.session_metas {
            if meta.user_count == 0 { continue; }
            if let Some(analysis) = self.analyze_session(&meta.session_id) {
                match analysis.grade_letter() {
                    "A" => grades[0] += 1,
                    "B" => grades[1] += 1,
                    "C" => grades[2] += 1,
                    "D" => grades[3] += 1,
                    _ => grades[4] += 1,
                }
            }
        }
        grades
    }

    /// Average cache hit rate across all sessions
    pub fn avg_cache_hit_rate(&self) -> f64 {
        let mut total = 0.0;
        let mut count = 0usize;
        for meta in &self.session_metas {
            if meta.user_count == 0 { continue; }
            if let Some(analysis) = self.analyze_session(&meta.session_id) {
                total += analysis.cache_hit_rate;
                count += 1;
            }
        }
        if count == 0 { 0.0 } else { total / count as f64 }
    }

    /// Total context growth premium (money wasted on bloated context)
    pub fn total_context_premium(&self) -> f64 {
        self.session_metas.iter()
            .filter(|s| s.user_count > 0)
            .filter_map(|s| self.analyze_session(&s.session_id))
            .map(|a| a.context_growth_premium)
            .sum()
    }

    /// Total compactions across all sessions
    pub fn total_compactions(&self) -> usize {
        self.session_metas.iter()
            .filter(|s| s.user_count > 0)
            .filter_map(|s| self.analyze_session(&s.session_id))
            .map(|a| a.compaction_count)
            .sum()
    }

    /// Session duration buckets: [<15m, 15-60m, 1-3h, 3h+]
    pub fn session_duration_buckets(&self) -> [usize; 4] {
        let mut buckets = [0usize; 4];
        for meta in &self.session_metas {
            if meta.user_count == 0 { continue; }
            let mins = (meta.end_time - meta.start_time).num_seconds() as f64 / 60.0;
            if mins <= 0.0 || mins > 10080.0 { continue; }
            if mins < 15.0 { buckets[0] += 1; }
            else if mins < 60.0 { buckets[1] += 1; }
            else if mins < 180.0 { buckets[2] += 1; }
            else { buckets[3] += 1; }
        }
        buckets
    }

    /// This week vs last week: returns (this_week_cost, last_week_cost, this_week_sessions, last_week_sessions)
    pub fn week_comparison(&self) -> (f64, f64, usize, usize) {
        let now = Utc::now();
        let week_ago = now - Duration::days(7);
        let two_weeks_ago = now - Duration::days(14);

        let this_week = self.aggregate_records(|r| r.timestamp >= week_ago);
        let last_week = self.aggregate_records(|r| r.timestamp >= two_weeks_ago && r.timestamp < week_ago);

        (this_week.cost, last_week.cost, this_week.session_count, last_week.session_count)
    }

    /// Monthly cost projection: (days_elapsed, daily_avg, projected_month_total)
    pub fn month_projection(&self) -> (u32, f64, f64) {
        use chrono::Datelike;
        let today = Local::now().date_naive();
        let day_of_month = today.day();
        let days_in_month = if today.month() == 12 {
            NaiveDate::from_ymd_opt(today.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
        }.unwrap_or(today).signed_duration_since(
            NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today)
        ).num_days() as u32;

        let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
        let month_cost: f64 = self.records.iter()
            .filter(|r| r.timestamp.with_timezone(&Local).date_naive() >= month_start)
            .map(|r| pricing::estimate_cost(&r.model, r.input_tokens, r.output_tokens,
                r.cache_creation_tokens, r.cache_read_tokens))
            .sum();

        let daily_avg = if day_of_month > 0 { month_cost / day_of_month as f64 } else { 0.0 };
        let projected = daily_avg * days_in_month as f64;

        (day_of_month, daily_avg, projected)
    }

    /// Personal records: most expensive session, highest output, highest cache hit
    pub fn personal_records(&self) -> Vec<(&'static str, String, String)> {
        let mut records: Vec<(&'static str, String, String)> = Vec::new();

        let mut max_cost: Option<(f64, String)> = None;
        let mut max_output: Option<(u64, String)> = None;
        let mut max_cache: Option<(f64, String)> = None;
        let mut max_tools: Option<(usize, String)> = None;

        for meta in &self.session_metas {
            if meta.user_count == 0 { continue; }
            let cost = self.session_cost(&meta.session_id);
            let (_, out) = self.session_tokens(&meta.session_id);
            let topic = if meta.first_message.chars().count() > 20 {
                let truncated: String = meta.first_message.chars().take(17).collect();
                format!("{}...", truncated)
            } else {
                meta.first_message.clone()
            };

            if max_cost.as_ref().is_none_or(|m| cost > m.0) {
                max_cost = Some((cost, topic.clone()));
            }
            if max_output.as_ref().is_none_or(|m| out > m.0) {
                max_output = Some((out, topic.clone()));
            }

            if let Some(analysis) = self.analyze_session(&meta.session_id) {
                if max_cache.as_ref().is_none_or(|m| analysis.cache_hit_rate > m.0) {
                    max_cache = Some((analysis.cache_hit_rate, topic.clone()));
                }
            }

            let tool_count: usize = meta.tool_counts.values().sum();
            if max_tools.as_ref().is_none_or(|m| tool_count > m.0) {
                max_tools = Some((tool_count, topic.clone()));
            }
        }

        if let Some((cost, topic)) = max_cost {
            records.push(("Priciest", pricing::format_cost(cost), topic));
        }
        if let Some((out, topic)) = max_output {
            records.push(("Most output", crate::tui::widgets::compact(out), topic));
        }
        if let Some((rate, _topic)) = max_cache {
            records.push(("Best cache", format!("{:.0}%", rate * 100.0), String::new()));
        }
        if let Some((count, topic)) = max_tools {
            records.push(("Most tools", format!("{}", count), topic));
        }

        records
    }

    /// Get subagents for a parent session, with their analysis
    pub fn subagents_for(&self, parent_id: &str) -> Vec<(&SessionMeta, Option<SessionAnalysis>)> {
        let mut subs: Vec<(&SessionMeta, Option<SessionAnalysis>)> = self.session_metas.iter()
            .filter(|s| s.parent_session_id.as_deref() == Some(parent_id))
            .map(|s| {
                let ana = self.analyze_session(&s.session_id);
                (s, ana)
            })
            .collect();
        subs.sort_by(|a, b| a.0.start_time.cmp(&b.0.start_time));
        subs
    }

    /// Model mix for a session including its subagents: Vec<(model_name, cost, pct)>
    pub fn session_model_mix(&self, session_id: &str) -> Vec<(String, f64, f64)> {
        let mut model_costs: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

        // Parent session
        for r in &self.records {
            if r.session_id == session_id {
                let model = simplify_model(&r.model);
                *model_costs.entry(model).or_default() += pricing::estimate_cost(
                    &r.model, r.input_tokens, r.output_tokens,
                    r.cache_creation_tokens, r.cache_read_tokens,
                );
            }
        }

        // Subagents
        for meta in &self.session_metas {
            if meta.parent_session_id.as_deref() == Some(session_id) {
                for r in &self.records {
                    if r.session_id == meta.session_id {
                        let model = simplify_model(&r.model);
                        *model_costs.entry(model).or_default() += pricing::estimate_cost(
                            &r.model, r.input_tokens, r.output_tokens,
                            r.cache_creation_tokens, r.cache_read_tokens,
                        );
                    }
                }
            }
        }

        let total: f64 = model_costs.values().sum();
        let mut mix: Vec<(String, f64, f64)> = model_costs.into_iter()
            .map(|(m, c)| {
                let pct = if total > 0.0 { c / total * 100.0 } else { 0.0 };
                (m, c, pct)
            })
            .collect();
        mix.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        mix
    }

    /// Output tokens per dollar (productivity metric)
    pub fn output_per_dollar(&self) -> f64 {
        let all = self.all_time();
        let total_output: u64 = self.records.iter().map(|r| r.output_tokens).sum();
        if all.cost > 0.0 { total_output as f64 / all.cost } else { 0.0 }
    }

    /// This month vs last month: (this_cost, last_cost, this_sessions, last_sessions)
    pub fn month_comparison(&self) -> (f64, f64, usize, usize) {
        let today = Local::now().date_naive();
        let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
        let last_month_start = if today.month() == 1 {
            NaiveDate::from_ymd_opt(today.year() - 1, 12, 1).unwrap_or(today)
        } else {
            NaiveDate::from_ymd_opt(today.year(), today.month() - 1, 1).unwrap_or(today)
        };

        let this_month = self.aggregate_records(|r| {
            r.timestamp.with_timezone(&Local).date_naive() >= month_start
        });
        let last_month = self.aggregate_records(|r| {
            let d = r.timestamp.with_timezone(&Local).date_naive();
            d >= last_month_start && d < month_start
        });

        (this_month.cost, last_month.cost, this_month.session_count, last_month.session_count)
    }

    /// Top tools across all sessions, returns Vec<(tool_name, count)> sorted desc
    pub fn top_tools(&self, limit: usize) -> Vec<(String, usize)> {
        let mut tool_map: HashMap<String, usize> = HashMap::new();
        for meta in &self.session_metas {
            for (tool, count) in &meta.tool_counts {
                *tool_map.entry(tool.clone()).or_default() += count;
            }
        }
        let mut tools: Vec<(String, usize)> = tool_map.into_iter().collect();
        tools.sort_by(|a, b| b.1.cmp(&a.1));
        tools.truncate(limit);
        tools
    }

    /// Average session duration in minutes
    pub fn avg_session_duration(&self) -> f64 {
        let durations: Vec<f64> = self.session_metas.iter()
            .filter(|s| s.user_count > 0)
            .map(|s| (s.end_time - s.start_time).num_seconds() as f64 / 60.0)
            .filter(|d| *d > 0.0 && *d < 10080.0) // filter out unreasonable durations (>1 week)
            .collect();
        if durations.is_empty() { return 0.0; }
        durations.iter().sum::<f64>() / durations.len() as f64
    }

    /// Full-text search across session JSONL content. Lazy: reads files on demand.
    /// Returns session IDs that contain the query in any user or assistant message.
    pub fn search_full_text(&self, query: &str) -> Vec<String> {
        let q = query.to_lowercase();
        let mut results: Vec<(String, chrono::DateTime<Utc>)> = Vec::new();

        for meta in &self.session_metas {
            if meta.source != Source::ClaudeCode { continue; }
            if meta.file_path.is_empty() { continue; }

            // Fast check: first_message might match
            if meta.first_message.to_lowercase().contains(&q) {
                results.push((meta.session_id.clone(), meta.start_time));
                continue;
            }

            // Lazy: read the file and search content
            if let Ok(content) = std::fs::read_to_string(&meta.file_path) {
                if content.to_lowercase().contains(&q) {
                    results.push((meta.session_id.clone(), meta.start_time));
                }
            }
        }

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.into_iter().map(|(id, _)| id).collect()
    }

    /// 7-day rolling average daily cost
    pub fn rolling_avg_daily_cost(&self, days: usize) -> f64 {
        let day_data = self.by_day(days);
        if day_data.len() < 3 { return 0.0; }
        let total: f64 = day_data.iter().map(|d| d.cost).sum();
        total / day_data.len() as f64
    }

    /// Today's savings (context growth premium) filtered by source
    #[allow(dead_code)]
    pub fn today_savings_by_source(&self, source: Source) -> f64 {
        let today = Local::now().date_naive();
        self.session_metas.iter()
            .filter(|s| s.source == source && s.start_time.with_timezone(&Local).date_naive() == today && !s.is_subagent)
            .filter_map(|s| self.analyze_session(&s.session_id))
            .map(|a| a.context_growth_premium)
            .sum()
    }

    /// Today's model usage breakdown
    #[allow(dead_code)]
    pub fn today_by_model(&self) -> Vec<ModelSummary> {
        let today = Local::now().date_naive();
        let mut map: HashMap<String, ModelSummary> = HashMap::new();
        for r in &self.records {
            if r.timestamp.with_timezone(&Local).date_naive() != today { continue; }
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

    /// Today's aggregation filtered by source
    pub fn today_by_source(&self, source: Source) -> Aggregation {
        let today = Local::now().date_naive();
        self.aggregate_records(|r| r.timestamp.with_timezone(&Local).date_naive() == today && r.source == source)
    }

    /// Count sessions by source for today
    pub fn today_sessions_by_source(&self, source: Source) -> Vec<&SessionMeta> {
        let today = Local::now().date_naive();
        let mut sessions: Vec<&SessionMeta> = self.session_metas.iter()
            .filter(|s| s.source == source && s.start_time.with_timezone(&Local).date_naive() == today)
            .collect();
        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        sessions
    }

    /// Today's cost and session count per hour (0..24), for the hourly heatmap
    pub fn today_by_hour(&self) -> Vec<(f64, usize)> {
        use chrono::Timelike;
        let today = Local::now().date_naive();
        let mut hours: Vec<(f64, HashSet<String>)> = (0..24).map(|_| (0.0, HashSet::new())).collect();
        for r in &self.records {
            let local_ts = r.timestamp.with_timezone(&Local);
            if local_ts.date_naive() != today { continue; }
            let h = local_ts.hour() as usize;
            if h < 24 {
                hours[h].0 += pricing::estimate_cost(
                    &r.model, r.input_tokens, r.output_tokens,
                    r.cache_creation_tokens, r.cache_read_tokens,
                );
                hours[h].1.insert(r.session_id.clone());
            }
        }
        hours.into_iter().map(|(cost, sessions)| (cost, sessions.len())).collect()
    }

    /// Projects sorted by cost (descending), all-time
    pub fn by_project_cost(&self) -> Vec<ProjectSummary> {
        let mut projects = self.by_project();
        projects.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));
        projects
    }

    /// Daily costs for last N days (for sparklines), ordered oldest to newest
    pub fn daily_costs(&self, days: usize) -> Vec<f64> {
        let day_data = self.by_day(days);
        let today = Utc::now().date_naive();
        let mut result = vec![0.0; days];
        for d in &day_data {
            let age = (today - d.date).num_days() as usize;
            if age < days {
                result[days - 1 - age] = d.cost;
            }
        }
        result
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
