use crate::parser::conversation::{SessionMeta, SessionStatus, SessionMode};
use crate::parser::{Source, UsageRecord};
use crate::pricing;
use chrono::{Datelike, Duration, Utc};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CursorModelStat {
    pub model: String,
    pub session_count: usize,
    pub completion_rate: f64,
    pub abort_rate: f64,
    pub avg_lines_added: f64,
    pub lines_per_1k_tokens: f64,
    pub avg_context_pct: f64,
}

struct CursorModelStatBuilder {
    model: String,
    session_count: usize,
    completed: usize,
    aborted: usize,
    total_lines_added: u64,
    total_output_tokens: u64,
    total_context_pct: f64,
    context_pct_count: usize,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct CursorOverviewStats {
    pub total_sessions: usize,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub total_lines: u64,
    pub total_files: u64,
    pub completion_rate: f64,
    pub abort_rate: f64,
    pub model_count: usize,
    pub days_active: u64,
    pub monthly_volumes: Vec<f64>,
    pub agent_pct: f64,
    pub avg_context_fill: f64,
    pub avg_lines_per_session: f64,
    pub cost_trend_pct: i64,
    pub this_week_cost: f64,
}

pub fn cursor_sessions<'a>(session_metas: &'a [SessionMeta]) -> Vec<&'a SessionMeta> {
    let mut sessions: Vec<&SessionMeta> = session_metas.iter()
        .filter(|s| s.source == Source::Cursor)
        .collect();
    sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    sessions
}

pub fn cursor_model_stats(session_metas: &[SessionMeta], records: &[UsageRecord]) -> Vec<CursorModelStat> {
    let sessions = cursor_sessions(session_metas);
    let mut model_map: HashMap<String, CursorModelStatBuilder> = HashMap::new();

    for session in &sessions {
        let model = records.iter()
            .find(|r| r.session_id == session.session_id && r.source == Source::Cursor)
            .map(|r| r.model.clone())
            .unwrap_or_default();
        if model.is_empty() { continue; }

        let builder = model_map.entry(model.clone()).or_insert_with(|| CursorModelStatBuilder {
            model: model.clone(),
            session_count: 0,
            completed: 0,
            aborted: 0,
            total_lines_added: 0,
            total_output_tokens: 0,
            total_context_pct: 0.0,
            context_pct_count: 0,
        });

        builder.session_count += 1;
        if session.cursor_status == Some(SessionStatus::Completed) { builder.completed += 1; }
        if session.cursor_status == Some(SessionStatus::Aborted) { builder.aborted += 1; }
        if let Some(la) = session.lines_added { builder.total_lines_added += la; }
        if let Some(pct) = session.context_usage_pct {
            builder.total_context_pct += pct;
            builder.context_pct_count += 1;
        }

        let out_tokens: u64 = records.iter()
            .filter(|r| r.session_id == session.session_id && r.source == Source::Cursor)
            .map(|r| r.output_tokens)
            .sum();
        builder.total_output_tokens += out_tokens;
    }

    let mut stats: Vec<CursorModelStat> = model_map.into_values().map(|b| {
        let non_empty = b.session_count.saturating_sub(
            b.session_count - b.completed - b.aborted
        ).max(1);
        let completion_rate = if non_empty > 0 {
            b.completed as f64 / non_empty as f64 * 100.0
        } else { 0.0 };
        let abort_rate = if non_empty > 0 {
            b.aborted as f64 / non_empty as f64 * 100.0
        } else { 0.0 };
        let avg_lines = if b.session_count > 0 {
            b.total_lines_added as f64 / b.session_count as f64
        } else { 0.0 };
        let lines_per_1k_tokens = if b.total_output_tokens > 0 {
            b.total_lines_added as f64 / (b.total_output_tokens as f64 / 1000.0)
        } else { 0.0 };
        let avg_context_pct = if b.context_pct_count > 0 {
            b.total_context_pct / b.context_pct_count as f64
        } else { 0.0 };

        CursorModelStat {
            model: b.model,
            session_count: b.session_count,
            completion_rate,
            abort_rate,
            avg_lines_added: avg_lines,
            lines_per_1k_tokens,
            avg_context_pct,
        }
    }).collect();

    stats.sort_by(|a, b| b.session_count.cmp(&a.session_count));
    stats
}

pub fn cursor_overview_stats(session_metas: &[SessionMeta], records: &[UsageRecord]) -> CursorOverviewStats {
    let sessions = cursor_sessions(session_metas);
    let cursor_records: Vec<&UsageRecord> = records.iter()
        .filter(|r| r.source == Source::Cursor)
        .collect();

    let total_sessions = sessions.len();
    let total_tokens: u64 = cursor_records.iter()
        .map(|r| r.input_tokens + r.output_tokens)
        .sum();
    let total_cost: f64 = cursor_records.iter()
        .map(|r| pricing::estimate_cost(&r.model, r.input_tokens, r.output_tokens,
            r.cache_creation_tokens, r.cache_read_tokens))
        .sum();
    let total_lines: u64 = sessions.iter().filter_map(|s| s.lines_added).sum();
    let total_files: u64 = sessions.iter().filter_map(|s| s.files_changed).sum();

    let completed = sessions.iter()
        .filter(|s| s.cursor_status == Some(SessionStatus::Completed))
        .count();
    let aborted = sessions.iter()
        .filter(|s| s.cursor_status == Some(SessionStatus::Aborted))
        .count();
    let non_empty = sessions.iter()
        .filter(|s| s.cursor_status != Some(SessionStatus::None))
        .count()
        .max(1);

    let completion_rate = completed as f64 / non_empty as f64 * 100.0;
    let abort_rate = aborted as f64 / non_empty as f64 * 100.0;

    let models: HashSet<&str> = cursor_records.iter()
        .map(|r| r.model.as_str())
        .filter(|m| !m.is_empty())
        .collect();
    let model_count = models.len();

    let mut sessions_by_month: HashMap<(i32, u32), usize> = HashMap::new();
    for s in &sessions {
        let key = (s.start_time.year(), s.start_time.month());
        *sessions_by_month.entry(key).or_default() += 1;
    }
    let now = Utc::now();
    let monthly_volumes: Vec<f64> = (0..7).rev().map(|i| {
        let date = now - Duration::days(i * 30);
        let key = (date.year(), date.month());
        *sessions_by_month.get(&key).unwrap_or(&0) as f64
    }).collect();

    let first_session = sessions.last().map(|s| s.start_time);
    let days_active = first_session.map(|f| (now - f).num_days() as u64).unwrap_or(0);

    let agent_count = sessions.iter()
        .filter(|s| s.cursor_mode == Some(SessionMode::Agent))
        .count();
    let agent_pct = if total_sessions > 0 {
        agent_count as f64 / total_sessions as f64 * 100.0
    } else { 0.0 };

    let ctx_pcts: Vec<f64> = sessions.iter()
        .filter_map(|s| s.context_usage_pct)
        .collect();
    let avg_context_fill = if !ctx_pcts.is_empty() {
        ctx_pcts.iter().sum::<f64>() / ctx_pcts.len() as f64
    } else { 0.0 };

    let avg_lines_per_session = if total_sessions > 0 {
        total_lines as f64 / total_sessions as f64
    } else { 0.0 };

    let week_ago = now - Duration::days(7);
    let two_weeks_ago = now - Duration::days(14);
    let this_week_cost: f64 = cursor_records.iter()
        .filter(|r| r.timestamp >= week_ago)
        .map(|r| pricing::estimate_cost(&r.model, r.input_tokens, r.output_tokens,
            r.cache_creation_tokens, r.cache_read_tokens))
        .sum();
    let last_week_cost: f64 = cursor_records.iter()
        .filter(|r| r.timestamp >= two_weeks_ago && r.timestamp < week_ago)
        .map(|r| pricing::estimate_cost(&r.model, r.input_tokens, r.output_tokens,
            r.cache_creation_tokens, r.cache_read_tokens))
        .sum();
    let cost_trend_pct = if last_week_cost > 0.0 {
        ((this_week_cost - last_week_cost) / last_week_cost * 100.0) as i64
    } else { 0 };

    CursorOverviewStats {
        total_sessions,
        total_tokens,
        total_cost,
        total_lines,
        total_files,
        completion_rate,
        abort_rate,
        model_count,
        days_active,
        monthly_volumes,
        agent_pct,
        avg_context_fill,
        avg_lines_per_session,
        cost_trend_pct,
        this_week_cost,
    }
}
