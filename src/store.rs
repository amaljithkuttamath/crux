use crate::parser::UsageRecord;
use crate::pricing;
use chrono::{Duration, NaiveDate, Timelike, Utc};
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

#[derive(Debug)]
pub struct SessionInsight {
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub message_count: usize,
    pub total_input: u64,
    pub total_output: u64,
    pub total_cache_read: u64,
    pub total_cache_write: u64,
    pub cost: f64,
    pub duration_minutes: i64,
    pub context_growth: Vec<u64>,  // input tokens per message, to show rot
}

#[derive(Debug)]
pub struct InsightsData {
    pub cache_hit_ratio: f64,         // 0.0-1.0
    pub output_efficiency: f64,       // output/input ratio
    pub avg_session_depth: f64,       // messages per session
    pub avg_cost_per_session: f64,
    pub cost_trend: f64,              // today's rate vs 7-day avg (1.0 = same, >1 = more)
    pub busiest_hours: Vec<(u8, u64)>, // (hour, token_count) top 3
    pub cache_waste_ratio: f64,       // cache writes with no reads
    pub context_rot_score: f64,       // avg growth factor of input tokens across session
    pub model_shift: Vec<(String, f64, f64)>, // (model, this_week_pct, last_week_pct)
    pub sessions: Vec<SessionInsight>, // recent sessions for detail
    pub daily_costs: Vec<f64>,        // last 7 days of costs for sparkline
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

    pub fn total_cost(&self) -> f64 {
        self.records.iter().map(|r| {
            pricing::estimate_cost(&r.model, r.input_tokens, r.output_tokens, r.cache_creation_tokens, r.cache_read_tokens)
        }).sum()
    }

    fn aggregate_records<'a>(&'a self, filter: impl Fn(&UsageRecord) -> bool) -> Aggregation {
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

    pub fn rolling_window(&self, duration: Duration) -> Aggregation {
        let cutoff = Utc::now() - duration;
        self.aggregate_records(|r| r.timestamp >= cutoff)
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

    pub fn by_model_window(&self, duration: Duration) -> Vec<ModelSummary> {
        let cutoff = Utc::now() - duration;
        let mut map: HashMap<String, ModelSummary> = HashMap::new();
        for r in &self.records {
            if r.timestamp < cutoff {
                continue;
            }
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

    pub fn burn_rate(&self, window: Duration) -> f64 {
        let agg = self.rolling_window(window);
        let hours = window.num_hours().max(1) as f64;
        agg.total_tokens() as f64 / hours
    }

    pub fn cost_rate(&self, window: Duration) -> f64 {
        let agg = self.rolling_window(window);
        let hours = window.num_hours().max(1) as f64;
        agg.cost / hours
    }

    pub fn avg_tokens_per_session(&self) -> u64 {
        let all = self.all_time();
        if all.session_count == 0 { return 0; }
        all.total_tokens() / all.session_count as u64
    }

    pub fn avg_cost_per_session(&self) -> f64 {
        let all = self.all_time();
        if all.session_count == 0 { return 0.0; }
        all.cost / all.session_count as f64
    }

    pub fn insights(&self) -> InsightsData {
        self.insights_with_days(7)
    }

    pub fn insights_with_days(&self, sparkline_days: usize) -> InsightsData {
        let all = self.all_time();

        // Cache hit ratio: cache_read / (cache_read + input)
        let cache_denom = all.cache_read_tokens + all.input_tokens;
        let cache_hit_ratio = if cache_denom > 0 {
            all.cache_read_tokens as f64 / cache_denom as f64
        } else {
            0.0
        };

        // Output efficiency: output / input
        let output_efficiency = if all.input_tokens > 0 {
            all.output_tokens as f64 / all.input_tokens as f64
        } else {
            0.0
        };

        // Cache waste: cache writes where session has low cache reads
        let mut session_cache_write: HashMap<String, u64> = HashMap::new();
        let mut session_cache_read: HashMap<String, u64> = HashMap::new();
        for r in &self.records {
            *session_cache_write.entry(r.session_id.clone()).or_default() += r.cache_creation_tokens;
            *session_cache_read.entry(r.session_id.clone()).or_default() += r.cache_read_tokens;
        }
        let mut total_writes = 0u64;
        let mut wasted_writes = 0u64;
        for (sid, writes) in &session_cache_write {
            total_writes += writes;
            let reads = session_cache_read.get(sid).copied().unwrap_or(0);
            if *writes > 0 && reads < *writes / 4 {
                wasted_writes += writes;
            }
        }
        let cache_waste_ratio = if total_writes > 0 {
            wasted_writes as f64 / total_writes as f64
        } else {
            0.0
        };

        // Session-level analysis
        let mut session_records: HashMap<String, Vec<&UsageRecord>> = HashMap::new();
        for r in &self.records {
            session_records.entry(r.session_id.clone()).or_default().push(r);
        }

        let mut sessions: Vec<SessionInsight> = Vec::new();
        let mut total_depth = 0usize;
        let mut total_sessions = 0usize;
        let mut context_growth_factors: Vec<f64> = Vec::new();

        for (sid, mut recs) in session_records {
            if sid.is_empty() { continue; }
            recs.sort_by_key(|r| r.timestamp);
            total_depth += recs.len();
            total_sessions += 1;

            let context_growth: Vec<u64> = recs.iter().map(|r| r.input_tokens).collect();

            // Context rot: how much does input grow from first to last message?
            if context_growth.len() >= 2 {
                let first = context_growth[0].max(1) as f64;
                let last = *context_growth.last().unwrap() as f64;
                context_growth_factors.push(last / first);
            }

            let first_ts = recs.first().map(|r| r.timestamp).unwrap_or_else(Utc::now);
            let last_ts = recs.last().map(|r| r.timestamp).unwrap_or_else(Utc::now);
            let duration_minutes = (last_ts - first_ts).num_minutes();

            let total_input: u64 = recs.iter().map(|r| r.input_tokens).sum();
            let total_output: u64 = recs.iter().map(|r| r.output_tokens).sum();
            let total_cache_read: u64 = recs.iter().map(|r| r.cache_read_tokens).sum();
            let total_cache_write: u64 = recs.iter().map(|r| r.cache_creation_tokens).sum();
            let cost: f64 = recs.iter().map(|r| {
                pricing::estimate_cost(&r.model, r.input_tokens, r.output_tokens,
                    r.cache_creation_tokens, r.cache_read_tokens)
            }).sum();

            let project = recs.first().map(|r| r.project.clone()).unwrap_or_default();
            let model = recs.first().map(|r| simplify_model(&r.model)).unwrap_or_default();

            sessions.push(SessionInsight {
                session_id: sid,
                project,
                model,
                message_count: recs.len(),
                total_input,
                total_output,
                total_cache_read,
                total_cache_write,
                cost,
                duration_minutes,
                context_growth,
            });
        }

        sessions.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));

        let avg_session_depth = if total_sessions > 0 {
            total_depth as f64 / total_sessions as f64
        } else {
            0.0
        };

        let avg_cost_per_session = if total_sessions > 0 {
            all.cost / total_sessions as f64
        } else {
            0.0
        };

        // Context rot score: average growth factor
        let context_rot_score = if context_growth_factors.is_empty() {
            1.0
        } else {
            context_growth_factors.iter().sum::<f64>() / context_growth_factors.len() as f64
        };

        // Cost trend: today's run rate vs 7-day average
        let today_cost = self.today().cost;
        let week = self.by_day(7);
        let avg_daily = if week.len() > 1 {
            week.iter().skip(1).map(|d| d.cost).sum::<f64>() / (week.len() - 1) as f64
        } else {
            today_cost
        };
        let cost_trend = if avg_daily > 0.0 {
            today_cost / avg_daily
        } else {
            1.0
        };

        // Busiest hours
        let mut hour_tokens: HashMap<u8, u64> = HashMap::new();
        for r in &self.records {
            let hour = r.timestamp.hour() as u8;
            *hour_tokens.entry(hour).or_default() += r.input_tokens + r.output_tokens;
        }
        let mut busiest_hours: Vec<(u8, u64)> = hour_tokens.into_iter().collect();
        busiest_hours.sort_by(|a, b| b.1.cmp(&a.1));
        busiest_hours.truncate(3);

        // Model shift: this week vs last week
        let now = Utc::now();
        let week_ago = now - Duration::days(7);
        let two_weeks_ago = now - Duration::days(14);

        let mut this_week_model: HashMap<String, usize> = HashMap::new();
        let mut last_week_model: HashMap<String, usize> = HashMap::new();
        let mut this_week_total = 0usize;
        let mut last_week_total = 0usize;

        for r in &self.records {
            let name = simplify_model(&r.model);
            if r.timestamp >= week_ago {
                *this_week_model.entry(name).or_default() += 1;
                this_week_total += 1;
            } else if r.timestamp >= two_weeks_ago {
                *last_week_model.entry(name).or_default() += 1;
                last_week_total += 1;
            }
        }

        let mut all_models: HashSet<String> = HashSet::new();
        all_models.extend(this_week_model.keys().cloned());
        all_models.extend(last_week_model.keys().cloned());

        let model_shift: Vec<(String, f64, f64)> = all_models.into_iter().map(|name| {
            let tw = if this_week_total > 0 {
                *this_week_model.get(&name).unwrap_or(&0) as f64 / this_week_total as f64 * 100.0
            } else { 0.0 };
            let lw = if last_week_total > 0 {
                *last_week_model.get(&name).unwrap_or(&0) as f64 / last_week_total as f64 * 100.0
            } else { 0.0 };
            (name, tw, lw)
        }).collect();

        // Daily costs for sparkline (oldest first)
        let days = self.by_day(sparkline_days);
        let mut daily_costs: Vec<f64> = days.iter().rev().map(|d| d.cost).collect();
        while daily_costs.len() < sparkline_days {
            daily_costs.insert(0, 0.0);
        }

        InsightsData {
            cache_hit_ratio,
            output_efficiency,
            avg_session_depth,
            avg_cost_per_session,
            cost_trend,
            busiest_hours,
            cache_waste_ratio,
            context_rot_score,
            model_shift,
            sessions,
            daily_costs,
        }
    }
}

fn simplify_model(model: &str) -> String {
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
