use crate::parser::UsageRecord;
use crate::pricing;
use chrono::Utc;
use serde;

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

impl SessionAnalysis {
    pub fn grade_letter(&self) -> &'static str {
        let mut score = 100i32;
        if self.context_growth > 8.0 { score -= 30; }
        else if self.context_growth > 5.0 { score -= 20; }
        else if self.context_growth > 3.0 { score -= 10; }
        if self.output_efficiency < 0.1 { score -= 30; }
        else if self.output_efficiency < 0.2 { score -= 15; }
        if self.cost_per_1k_output > 1.0 { score -= 20; }
        else if self.cost_per_1k_output > 0.5 { score -= 10; }
        if self.compaction_count > 0 { score += 5; }
        match score {
            90..=200 => "A",
            75..=89 => "B",
            60..=74 => "C",
            40..=59 => "D",
            _ => "F",
        }
    }
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
    pub context_size: u64,
    pub cost: f64,
    pub output_tokens: u64,
    pub is_compaction: bool,
    pub context_pct: f64,
}

/// Timeline of a session: turns + notable events
#[derive(Debug, Clone)]
pub struct SessionTimeline {
    pub turns: Vec<TurnSnapshot>,
    pub total_cost: f64,
    pub duration_minutes: i64,
    pub compaction_count: usize,
}

pub fn analyze_session(records: &[UsageRecord], session_id: &str) -> Option<SessionAnalysis> {
    let mut recs: Vec<&UsageRecord> = records
        .iter()
        .filter(|r| r.session_id == session_id)
        .collect();
    if recs.is_empty() { return None; }
    recs.sort_by_key(|r| r.timestamp);

    let project = recs[0].project.clone();
    let model = super::simplify_model(&recs[0].model);

    let contexts: Vec<u64> = recs.iter()
        .map(|r| r.cache_read_tokens + r.cache_creation_tokens)
        .collect();
    let context_initial = contexts[0].max(1);
    let context_current = *contexts.last().unwrap_or(&1);
    let context_peak = contexts.iter().copied().max().unwrap_or(1);
    let context_growth = context_current as f64 / context_initial as f64;

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

    let total_input: u64 = recs.iter().map(|r| r.input_tokens).sum();
    let total_output: u64 = recs.iter().map(|r| r.output_tokens).sum();
    let total_cache_read: u64 = recs.iter().map(|r| r.cache_read_tokens).sum();
    let total_cache_write: u64 = recs.iter().map(|r| r.cache_creation_tokens).sum();

    let p = pricing::pricing_for_model(&recs[0].model);
    let cost_cr = total_cache_read as f64 / 1e6 * p.cache_read_per_m;
    let cost_cw = total_cache_write as f64 / 1e6 * p.cache_write_per_m;
    let cost_in = total_input as f64 / 1e6 * p.input_per_m;
    let cost_out = total_output as f64 / 1e6 * p.output_per_m;
    let total_cost = cost_cr + cost_cw + cost_in + cost_out;

    let cache_denom = total_cache_read + total_input;
    let cache_hit_rate = if cache_denom > 0 {
        total_cache_read as f64 / cache_denom as f64
    } else { 0.0 };

    let total_context: u64 = recs.iter()
        .map(|r| r.cache_read_tokens + r.cache_creation_tokens)
        .sum();
    let output_efficiency = if total_context > 0 {
        total_output as f64 / total_context as f64 * 100.0
    } else { 0.0 };

    let cost_per_1k_output = if total_output > 0 {
        total_cost / (total_output as f64 / 1000.0)
    } else { 0.0 };

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

pub fn session_timeline(records: &[UsageRecord], session_id: &str, ceiling: Option<u64>) -> Option<SessionTimeline> {
    let mut recs: Vec<&UsageRecord> = records
        .iter()
        .filter(|r| r.session_id == session_id)
        .collect();
    if recs.is_empty() { return None; }
    recs.sort_by_key(|r| r.timestamp);

    let ceiling_f = ceiling.map(|c| c as f64);
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
            context_pct: ceiling_f.map(|c| (ctx as f64 / c * 100.0).min(100.0)).unwrap_or(0.0),
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthStatus {
    Fresh,
    Healthy,
    Aging,
    CtxRot,
    Done,
    #[allow(dead_code)]
    Aborted,
}

impl HealthStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Healthy => "healthy",
            Self::Aging => "aging",
            Self::CtxRot => "ctx rot",
            Self::Done => "done",
            Self::Aborted => "aborted",
        }
    }

    pub fn sort_order(&self) -> u8 {
        match self {
            Self::CtxRot => 5,
            Self::Aging => 4,
            Self::Healthy => 3,
            Self::Fresh => 2,
            Self::Done => 1,
            Self::Aborted => 0,
        }
    }
}

/// Compute health status for a session.
/// When ceiling is known, use context fill percentage.
/// When unknown, use multi-factor analysis (growth, efficiency, compactions).
pub fn health_status(
    analysis: &SessionAnalysis,
    ceiling: Option<u64>,
    is_live: bool,
    warn_pct: f64,
    danger_pct: f64,
) -> HealthStatus {
    if !is_live { return HealthStatus::Done; }

    if let Some(ceil) = ceiling {
        let fill_pct = analysis.context_current as f64 / ceil as f64 * 100.0;
        if fill_pct >= danger_pct { return HealthStatus::CtxRot; }
        if fill_pct >= warn_pct { return HealthStatus::Aging; }
        if fill_pct < warn_pct * 0.6 { return HealthStatus::Fresh; }
        return HealthStatus::Healthy;
    }

    // No ceiling: use multi-factor heuristics
    if analysis.context_growth > 6.0 && analysis.output_efficiency < 0.1 {
        return HealthStatus::CtxRot;
    }
    if analysis.context_growth > 4.0 && analysis.messages_since_compaction > 30 {
        return HealthStatus::Aging;
    }
    if analysis.context_growth < 2.0 { return HealthStatus::Fresh; }
    HealthStatus::Healthy
}
