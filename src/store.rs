use crate::parser::UsageRecord;
use crate::parser::conversation::SessionMeta;
use crate::pricing;
use chrono::{Duration, NaiveDate, Timelike, Utc};
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionAnalysis {
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub message_count: usize,
    pub total_cost: f64,
    pub cost_breakdown: CostBreakdown,
    pub context_current: u64,
    pub context_initial: u64,
    pub context_peak: u64,
    pub context_growth: f64,
    pub cache_hit_rate: f64,
    pub output_efficiency: f64,
    pub compaction_count: usize,
    pub cost_per_1k_output: f64,
    pub context_growth_premium: f64,
    pub messages_since_compaction: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CostBreakdown {
    pub cache_read: f64,
    pub cache_write: f64,
    pub input: f64,
    pub output: f64,
}

/// Per-turn snapshot for session timeline view
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TurnSnapshot {
    pub turn_index: usize,
    pub timestamp: chrono::DateTime<Utc>,
    pub context_size: u64,       // cache_read + cache_creation (proxy for context window fill)
    pub cost: f64,
    pub output_tokens: u64,
    pub is_compaction: bool,     // context dropped significantly from previous turn
    pub context_pct: f64,        // percentage of 167K ceiling
}

/// Timeline of a session: turns + notable events
#[derive(Debug, Clone)]
pub struct SessionTimeline {
    pub turns: Vec<TurnSnapshot>,
    pub total_cost: f64,
    pub duration_minutes: i64,
    pub compaction_count: usize,
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

    /// Sessions active in the last N hours, with full analysis
    pub fn active_sessions(&self, hours: i64) -> Vec<(&SessionMeta, SessionAnalysis)> {
        let cutoff = Utc::now() - Duration::hours(hours);
        let mut active: Vec<(&SessionMeta, SessionAnalysis)> = self.session_metas.iter()
            .filter(|s| s.end_time >= cutoff && s.user_count > 0)
            .filter_map(|s| {
                let analysis = self.analyze_session(&s.session_id)?;
                Some((s, analysis))
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

    /// Get cost for a specific session from usage records
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn cost_rate(&self, window: Duration) -> f64 {
        let agg = self.rolling_window(window);
        let hours = window.num_hours().max(1) as f64;
        agg.cost / hours
    }

    #[allow(dead_code)]
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

    pub fn analyze_session(&self, session_id: &str) -> Option<SessionAnalysis> {
        let mut recs: Vec<&UsageRecord> = self.records
            .iter()
            .filter(|r| r.session_id == session_id)
            .collect();
        if recs.is_empty() { return None; }
        recs.sort_by_key(|r| r.timestamp);

        let project = recs[0].project.clone();
        let model = simplify_model(&recs[0].model);

        // Context trajectory (cache_read + cache_creation per message)
        let contexts: Vec<u64> = recs.iter()
            .map(|r| r.cache_read_tokens + r.cache_creation_tokens)
            .collect();
        let context_initial = contexts[0].max(1);
        let context_current = *contexts.last().unwrap_or(&1);
        let context_peak = contexts.iter().copied().max().unwrap_or(1);
        let context_growth = context_current as f64 / context_initial as f64;

        // Compaction detection (context drops > 10K tokens)
        let mut compaction_count = 0usize;
        let mut last_compaction_idx = 0usize;
        for i in 1..contexts.len() {
            if contexts[i] < contexts[i-1].saturating_sub(10_000) {
                compaction_count += 1;
                last_compaction_idx = i;
            }
        }
        let messages_since_compaction = if compaction_count > 0 {
            contexts.len() - last_compaction_idx
        } else {
            contexts.len()
        };

        // Totals
        let total_input: u64 = recs.iter().map(|r| r.input_tokens).sum();
        let total_output: u64 = recs.iter().map(|r| r.output_tokens).sum();
        let total_cache_read: u64 = recs.iter().map(|r| r.cache_read_tokens).sum();
        let total_cache_write: u64 = recs.iter().map(|r| r.cache_creation_tokens).sum();

        // Cost breakdown
        let p = pricing::pricing_for_model(&recs[0].model);
        let cost_cr = total_cache_read as f64 / 1e6 * p.cache_read_per_m;
        let cost_cw = total_cache_write as f64 / 1e6 * p.cache_write_per_m;
        let cost_in = total_input as f64 / 1e6 * p.input_per_m;
        let cost_out = total_output as f64 / 1e6 * p.output_per_m;
        let total_cost = cost_cr + cost_cw + cost_in + cost_out;

        // Cache hit rate
        let cache_denom = total_cache_read + total_input;
        let cache_hit_rate = if cache_denom > 0 {
            total_cache_read as f64 / cache_denom as f64
        } else { 0.0 };

        // Output efficiency (output / total context as percentage)
        let total_context: u64 = recs.iter()
            .map(|r| r.cache_read_tokens + r.cache_creation_tokens)
            .sum();
        let output_efficiency = if total_context > 0 {
            total_output as f64 / total_context as f64 * 100.0
        } else { 0.0 };

        // Cost per 1K output
        let cost_per_1k_output = if total_output > 0 {
            total_cost / (total_output as f64 / 1000.0)
        } else { 0.0 };

        // Context growth premium: actual cost vs hypothetical fresh-context cost
        let hypothetical_cr = context_initial * recs.len() as u64;
        let fresh_cost = hypothetical_cr as f64 / 1e6 * p.cache_read_per_m
            + cost_cw + cost_in + cost_out;
        let context_growth_premium = (total_cost - fresh_cost).max(0.0);

        Some(SessionAnalysis {
            session_id: session_id.to_string(),
            project,
            model,
            message_count: recs.len(),
            total_cost,
            cost_breakdown: CostBreakdown {
                cache_read: cost_cr,
                cache_write: cost_cw,
                input: cost_in,
                output: cost_out,
            },
            context_current,
            context_initial,
            context_peak,
            context_growth,
            cache_hit_rate,
            output_efficiency,
            compaction_count,
            cost_per_1k_output,
            context_growth_premium,
            messages_since_compaction,
        })
    }

    /// Build a timeline of per-turn snapshots for session detail view
    pub fn session_timeline(&self, session_id: &str) -> Option<SessionTimeline> {
        let mut recs: Vec<&UsageRecord> = self.records
            .iter()
            .filter(|r| r.session_id == session_id)
            .collect();
        if recs.is_empty() { return None; }
        recs.sort_by_key(|r| r.timestamp);

        let ceiling = 167_000.0f64;
        let mut turns = Vec::new();
        let mut compaction_count = 0usize;
        let mut prev_ctx = 0u64;

        for (i, r) in recs.iter().enumerate() {
            let ctx = r.cache_read_tokens + r.cache_creation_tokens;
            let is_compaction = i > 0 && ctx < prev_ctx.saturating_sub(10_000);
            if is_compaction { compaction_count += 1; }

            let cost = pricing::estimate_cost(
                &r.model, r.input_tokens, r.output_tokens,
                r.cache_creation_tokens, r.cache_read_tokens,
            );

            turns.push(TurnSnapshot {
                turn_index: i,
                timestamp: r.timestamp,
                context_size: ctx,
                cost,
                output_tokens: r.output_tokens,
                is_compaction,
                context_pct: (ctx as f64 / ceiling * 100.0).min(100.0),
            });
            prev_ctx = ctx;
        }

        let total_cost = turns.iter().map(|t| t.cost).sum();
        let duration_minutes = if turns.len() >= 2 {
            (turns.last().unwrap().timestamp - turns[0].timestamp).num_minutes()
        } else {
            0
        };

        Some(SessionTimeline {
            turns,
            total_cost,
            duration_minutes,
            compaction_count,
        })
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

    /// Count consecutive days (ending today or yesterday) with at least one session
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

    /// Sessions per day for the last N days (oldest first), for sparkline
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

    /// Token volume per hour of day (0-23), aggregated across all time
    #[allow(dead_code)]
    pub fn by_hour_all(&self) -> [u64; 24] {
        let mut hours = [0u64; 24];
        for r in &self.records {
            let h = r.timestamp.hour() as usize;
            hours[h] += r.input_tokens + r.output_tokens;
        }
        hours
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
