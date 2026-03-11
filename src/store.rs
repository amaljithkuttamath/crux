use crate::parser::UsageRecord;
use chrono::{Duration, NaiveDate, Utc};
use std::collections::{HashMap, HashSet};

pub struct Store {
    records: Vec<UsageRecord>,
}

#[derive(Debug, Default)]
pub struct Aggregation {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub session_count: usize,
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
}

#[derive(Debug)]
pub struct DaySummary {
    pub date: NaiveDate,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub session_count: usize,
}

impl Aggregation {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }
}

impl Store {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn add(&mut self, record: UsageRecord) {
        self.records.push(record);
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn rolling_window(&self, duration: Duration) -> Aggregation {
        let cutoff = Utc::now() - duration;
        let mut agg = Aggregation::default();
        let mut sessions = HashSet::new();
        for r in &self.records {
            if r.timestamp >= cutoff {
                agg.input_tokens += r.input_tokens;
                agg.output_tokens += r.output_tokens;
                agg.cache_creation_tokens += r.cache_creation_tokens;
                agg.cache_read_tokens += r.cache_read_tokens;
                sessions.insert(&r.session_id);
            }
        }
        agg.session_count = sessions.len();
        agg
    }

    pub fn today(&self) -> Aggregation {
        let today = Utc::now().date_naive();
        let mut agg = Aggregation::default();
        let mut sessions = HashSet::new();
        for r in &self.records {
            if r.timestamp.date_naive() == today {
                agg.input_tokens += r.input_tokens;
                agg.output_tokens += r.output_tokens;
                agg.cache_creation_tokens += r.cache_creation_tokens;
                agg.cache_read_tokens += r.cache_read_tokens;
                sessions.insert(&r.session_id);
            }
        }
        agg.session_count = sessions.len();
        agg
    }

    pub fn yesterday(&self) -> Aggregation {
        let yesterday = (Utc::now() - Duration::days(1)).date_naive();
        let mut agg = Aggregation::default();
        let mut sessions = HashSet::new();
        for r in &self.records {
            if r.timestamp.date_naive() == yesterday {
                agg.input_tokens += r.input_tokens;
                agg.output_tokens += r.output_tokens;
                agg.cache_creation_tokens += r.cache_creation_tokens;
                agg.cache_read_tokens += r.cache_read_tokens;
                sessions.insert(&r.session_id);
            }
        }
        agg.session_count = sessions.len();
        agg
    }

    pub fn this_week(&self) -> Aggregation {
        let cutoff = Utc::now() - Duration::days(7);
        let mut agg = Aggregation::default();
        let mut sessions = HashSet::new();
        for r in &self.records {
            if r.timestamp >= cutoff {
                agg.input_tokens += r.input_tokens;
                agg.output_tokens += r.output_tokens;
                agg.cache_creation_tokens += r.cache_creation_tokens;
                agg.cache_read_tokens += r.cache_read_tokens;
                sessions.insert(&r.session_id);
            }
        }
        agg.session_count = sessions.len();
        agg
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
            });
            entry.input_tokens += r.input_tokens;
            entry.output_tokens += r.output_tokens;
            entry.cache_creation_tokens += r.cache_creation_tokens;
            entry.cache_read_tokens += r.cache_read_tokens;
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
            });
            entry.input_tokens += r.input_tokens;
            entry.output_tokens += r.output_tokens;
            entry.cache_creation_tokens += r.cache_creation_tokens;
            entry.cache_read_tokens += r.cache_read_tokens;
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

    pub fn burn_rate(&self, window: Duration) -> f64 {
        let agg = self.rolling_window(window);
        let hours = window.num_hours().max(1) as f64;
        agg.total_tokens() as f64 / hours
    }
}
