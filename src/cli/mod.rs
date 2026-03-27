pub mod statusline;
pub mod widget;

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

pub fn format_stats(store: &Store) -> String {
    let all = store.all_time();
    let streak = store.streak_days();
    let longest_streak = store.longest_streak();
    let active_days = store.active_days();
    let total_tokens = store.total_tokens();
    let favorite_model = store.favorite_model().unwrap_or_else(|| "none".to_string());
    let avg_dur = store.avg_session_duration();
    let night_ratio = store.night_owl_ratio();

    let mut out = String::new();
    out.push_str(&format!("Sessions: {}  Active days: {}  Tokens: {}\n", all.session_count, active_days, compact(total_tokens)));
    out.push_str(&format!("Current streak: {} days  Longest streak: {} days\n", streak, longest_streak));
    out.push_str(&format!("Favorite model: {}  Total cost: {}\n", favorite_model, pricing::format_cost(all.cost)));
    out.push_str(&format!("Avg session: {:.0}m  Night owl: {:.0}%\n", avg_dur, night_ratio));

    if let Some((_, mins)) = store.longest_session() {
        let dur = if mins >= 1440.0 {
            format!("{}d {}h", (mins / 1440.0).floor() as u64, ((mins % 1440.0) / 60.0).floor() as u64)
        } else if mins >= 60.0 {
            format!("{}h {}m", (mins / 60.0).floor() as u64, (mins % 60.0).floor() as u64)
        } else {
            format!("{:.0}m", mins)
        };
        out.push_str(&format!("Longest session: {}\n", dur));
    }

    if let Some((date, count)) = store.most_active_day() {
        out.push_str(&format!("Most active day: {} ({} sessions)\n", date, count));
    }

    // Heatmap
    let (grid, _) = store.activity_heatmap();
    let total_days = grid.len();
    let weeks = total_days.div_ceil(7);
    let max_count = grid.iter().max().copied().unwrap_or(1).max(1);
    let blocks = ['\u{00b7}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2588}'];
    let day_labels = ["   ", "Mon", "   ", "Wed", "   ", "Fri", "   "];

    out.push('\n');
    for (row, label) in day_labels.iter().enumerate() {
        out.push_str(&format!("{} ", label));
        for week in 0..weeks {
            let idx = week * 7 + row;
            if idx >= total_days {
                out.push('\u{00b7}');
                continue;
            }
            let count = grid[idx];
            let intensity = if count == 0 { 0 } else {
                ((count as f64 / max_count as f64) * 3.0).ceil() as usize + 1
            };
            out.push(blocks[intensity.min(4)]);
        }
        out.push('\n');
    }
    out.push_str("    Less \u{2591} \u{2592} \u{2593} \u{2588} More\n");

    // Efficiency
    let cache_rate = store.avg_cache_hit_rate();
    let premium = store.total_context_premium();
    let compactions = store.total_compactions();
    out.push_str(&format!("\nCache hit: {:.0}%  Ctx bloat: {}  Compactions: {}\n",
        cache_rate * 100.0, pricing::format_cost(premium), compactions));

    // Grade distribution
    let grades = store.grade_distribution();
    out.push_str(&format!("Grades: A:{} B:{} C:{} D:{} F:{}\n",
        grades[0], grades[1], grades[2], grades[3], grades[4]));

    // Week comparison
    let (tw, lw, tws, _lws) = store.week_comparison();
    let delta = if lw > 0.0 { (tw - lw) / lw * 100.0 } else { 0.0 };
    out.push_str(&format!("This week: {} ({} sess)  {}{:.0}% vs last\n",
        pricing::format_cost(tw), tws,
        if delta >= 0.0 { "+" } else { "" }, delta));

    // Month comparison
    let (tm, lm, tms, _lms) = store.month_comparison();
    let mdelta = if lm > 0.0 { (tm - lm) / lm * 100.0 } else { 0.0 };
    out.push_str(&format!("This month: {} ({} sess)  {}{:.0}% vs last\n",
        pricing::format_cost(tm), tms,
        if mdelta >= 0.0 { "+" } else { "" }, mdelta));

    // Top tools
    let tools = store.top_tools(5);
    if !tools.is_empty() {
        out.push_str("\nTop tools: ");
        for (i, (name, count)) in tools.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            out.push_str(&format!("{}({})", name, count));
        }
        out.push('\n');
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
