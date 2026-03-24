use crate::config::Config;
use crate::parser::{self, Source};
use crate::store::Store;
use chrono::Utc;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct WidgetData {
    pub generated_at: String,
    pub today: TodaySummary,
    pub active_sessions: Vec<ActiveSession>,
}

#[derive(Serialize)]
pub struct TodaySummary {
    pub total_cost: f64,
    pub burn_rate_per_hour: f64,
    pub sources: SourceCosts,
}

#[derive(Serialize)]
pub struct SourceCosts {
    pub claude_code: f64,
    pub cursor: f64,
}

#[derive(Serialize)]
pub struct ActiveSession {
    pub session_id: String,
    pub project: String,
    pub source: String,
    pub model: String,
    pub duration_minutes: i64,
    pub cost: f64,
    pub health_grade: String,
    pub context_percent: f64,
}

pub fn build_widget_data(store: &Store, _config: &Config) -> WidgetData {
    let today = store.today();
    let cc_today = store.today_by_source(Source::ClaudeCode);
    let cu_today = store.today_by_source(Source::Cursor);

    // Liveness check
    let sessions_dir = dirs::home_dir().unwrap_or_default().join(".claude/sessions");
    let live_map = parser::liveness::check_liveness(&sessions_dir);

    // Build active sessions: live CC sessions + recent Cursor sessions
    let mut active_sessions = Vec::new();

    for meta in store.sessions_by_time() {
        let is_live = live_map.get(&meta.session_id).copied().unwrap_or(false);

        // For Cursor, no PID check available. Use recency (active in last 10 min).
        let is_cursor_active = meta.source == Source::Cursor
            && (Utc::now() - meta.end_time).num_minutes() < 10;

        if !is_live && !is_cursor_active {
            continue;
        }

        let analysis = match store.analyze_session(&meta.session_id) {
            Some(a) => a,
            None => continue,
        };

        // Duration: for live sessions, use now - start_time
        let duration = (Utc::now() - meta.start_time).num_minutes();

        let source_str = match meta.source {
            Source::ClaudeCode => "claude_code",
            Source::Cursor => "cursor",
        };

        // Check if this session is at a workspace root by looking at its
        // file path. The parent directory encodes the full working dir:
        // e.g. -Users-amal-Developer-lab-crux -> has a project sub-path
        //      -Users-amal-Developer          -> workspace root, no project
        // A root directory has the same name as meta.project after display_project_name
        // processes it. We detect root sessions by checking if the raw directory
        // name (from file_path) has no path component after the last known
        // home directory segment. Direct check: re-run extract on the dir name
        // and see if it produced a single-segment result matching the dir's tail.
        let display_name = crate::tui::widgets::display_project_name(&meta.project);
        let is_root_session = {
            // The raw dir name from the JSONL path's parent
            let raw_dir = std::path::Path::new(&meta.file_path)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            // Root sessions: raw dir ends with a home path component and nothing after.
            // e.g. "-Users-amal-Developer" has no further segments after the last
            // recognizable path component. We check: does the raw dir, after the
            // last occurrence of the display_name, have nothing else?
            // Simplest direct check: raw dir ends with &meta.project and
            // meta.project equals display_name (no stripping happened).
            raw_dir.ends_with(&format!("-{}", &meta.project))
                && meta.project == display_name
                && !meta.project.is_empty()
        };

        let project_name = if is_root_session && !meta.first_message.is_empty() {
            crate::tui::widgets::truncate(&meta.first_message, 24)
        } else {
            display_name
        };

        active_sessions.push(ActiveSession {
            session_id: meta.session_id.clone(),
            project: project_name,
            source: source_str.to_string(),
            model: analysis.model.clone(),
            duration_minutes: duration.max(1),
            cost: analysis.total_cost,
            health_grade: analysis.grade_letter().to_string(),
            context_percent: analysis.context_pct(meta.context_token_limit),
        });
    }

    WidgetData {
        generated_at: Utc::now().to_rfc3339(),
        today: TodaySummary {
            total_cost: today.cost,
            burn_rate_per_hour: store.burn_rate(),
            sources: SourceCosts {
                claude_code: cc_today.cost,
                cursor: cu_today.cost,
            },
        },
        active_sessions,
    }
}

fn widget_json_path() -> PathBuf {
    let cache_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".cache/crux");
    std::fs::create_dir_all(&cache_dir).ok();
    cache_dir.join("widget.json")
}

/// Write widget.json atomically (write tmp, rename)
fn write_widget_json(data: &WidgetData) -> anyhow::Result<()> {
    let path = widget_json_path();
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// One-shot export
pub fn export_once(store: &Store, config: &Config) -> anyhow::Result<()> {
    let data = build_widget_data(store, config);
    write_widget_json(&data)?;
    let path = widget_json_path();
    eprintln!("Wrote {}", path.display());
    Ok(())
}

/// Watch mode: re-export every 60s with fresh data
pub fn export_watch(config: &Config) -> anyhow::Result<()> {
    let path = widget_json_path();
    eprintln!("Watch mode: writing to {} every 60s (Ctrl+C to stop)", path.display());

    loop {
        // Reload store each cycle to pick up new sessions
        let store = crate::load_store(config)?;
        let data = build_widget_data(&store, config);
        write_widget_json(&data)?;
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
