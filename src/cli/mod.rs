use crate::parser::Source;
use crate::pricing;
use crate::store::Store;
use crate::tui::widgets::{compact, format_ago};

pub fn format_summary(store: &Store) -> String {
    let today = store.today();
    let cc_today = store.today_by_source(Source::ClaudeCode);
    let cu_today = store.today_by_source(Source::Cursor);

    let cc_sessions = store.today_sessions_by_source(Source::ClaudeCode).len();
    let cu_sessions = store.today_sessions_by_source(Source::Cursor).len();

    let out = format!(
        "today: {} across {} sessions (Claude Code: {}/{}, Cursor: {}/{})\n",
        pricing::format_cost(today.cost),
        today.session_count,
        pricing::format_cost(cc_today.cost),
        cc_sessions,
        pricing::format_cost(cu_today.cost),
        cu_sessions,
    );

    out
}

pub fn format_daily(store: &Store, days: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<12} {:>8} {:>10} {:>10} {:>12} {:>8}\n",
        "Date", "Cost", "Input", "Output", "Cache", "Sessions"
    ));
    out.push_str(&"-".repeat(64));
    out.push('\n');
    for day in store.by_day(days) {
        out.push_str(&format!(
            "{:<12} {:>8} {:>10} {:>10} {:>12} {:>8}\n",
            day.date,
            pricing::format_cost(day.cost),
            compact(day.input_tokens),
            compact(day.output_tokens),
            compact(day.cache_creation_tokens + day.cache_read_tokens),
            day.session_count,
        ));
    }
    out
}

pub fn format_projects(store: &Store) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<28} {:>8} {:>10} {:>12}\n",
        "Project", "Cost", "Sessions", "Last Used"
    ));
    out.push_str(&"-".repeat(62));
    out.push('\n');
    for p in store.by_project() {
        let ago = format_ago(p.last_used);
        out.push_str(&format!(
            "{:<28} {:>8} {:>10} {:>12}\n",
            crate::tui::widgets::display_project_name(&p.name),
            pricing::format_cost(p.cost),
            p.session_count,
            ago,
        ));
    }
    out
}

pub fn format_sessions(store: &Store) -> String {
    let mut out = String::new();
    let sessions = store.sessions_by_time();

    out.push_str(&format!(
        "{:<6} {:<24} {:>6} {:>8} {:>3} {:>4}\n",
        "Time", "Topic", "Dur", "Cost", "Gr", "Src"
    ));
    out.push_str(&"-".repeat(56));
    out.push('\n');

    for session in sessions.iter().take(30) {
        let time_str = session.start_time.format("%H:%M").to_string();
        let topic = crate::tui::widgets::truncate(&session.first_message, 24);
        let dur = session.duration_minutes();
        let dur_str = if dur >= 60 {
            format!("{}h{:02}m", dur / 60, dur % 60)
        } else {
            format!("{}m", dur.max(1))
        };
        let cost = store.session_cost(&session.session_id);
        let grade = store.analyze_session(&session.session_id)
            .map(|a| a.grade_letter())
            .unwrap_or("-");
        let src = match session.source {
            Source::ClaudeCode => "CC",
            Source::Cursor => "Cu",
        };

        out.push_str(&format!(
            "{:<6} {:<24} {:>6} {:>8} {:>3} {:>4}\n",
            time_str,
            topic,
            dur_str,
            pricing::format_cost(cost),
            grade,
            src,
        ));
    }
    out
}

pub fn format_health(store: &Store) -> String {
    let active = store.active_sessions(24);
    if active.is_empty() {
        return "no active sessions\n".to_string();
    }

    let mut out = String::new();
    for (meta, analysis) in &active {
        let ctx_pct = (analysis.context_current as f64 / 167_000.0 * 100.0).min(100.0);
        let health = if ctx_pct > 85.0 {
            "CRITICAL"
        } else if ctx_pct > 70.0 && analysis.context_growth > 4.0 {
            "AGING"
        } else if ctx_pct < 40.0 {
            "FRESH"
        } else {
            "OK"
        };

        let topic = crate::tui::widgets::truncate(&meta.first_message, 30);
        out.push_str(&format!(
            "{:<8} {:<30} ctx:{:.0}% growth:{:.1}x cost:{}\n",
            health,
            topic,
            ctx_pct,
            analysis.context_growth,
            pricing::format_cost(analysis.total_cost),
        ));
    }
    out
}
