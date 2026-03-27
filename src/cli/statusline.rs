use crate::cli::widget::WidgetData;
use crate::config::Config;

const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

const BAR_WIDTH: usize = 10;

#[derive(serde::Deserialize)]
struct CcSessionInput {
    session_id: Option<String>,
    cost: Option<CostInfo>,
    context_window: Option<ContextInfo>,
}

#[derive(serde::Deserialize)]
struct CostInfo {
    total_cost_usd: Option<f64>,
}

#[derive(serde::Deserialize)]
struct ContextInfo {
    used_percentage: Option<f64>,
}

fn read_widget_cache() -> Option<WidgetData> {
    let path = dirs::home_dir()?.join(".cache/crux/widget.json");
    let metadata = std::fs::metadata(&path).ok()?;
    let age = metadata.modified().ok()?.elapsed().ok()?;
    if age > std::time::Duration::from_secs(120) {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn read_statusline_cache() -> Option<WidgetData> {
    let path = dirs::home_dir()?.join(".cache/crux/statusline-cache.json");
    let metadata = std::fs::metadata(&path).ok()?;
    let age = metadata.modified().ok()?.elapsed().ok()?;
    if age > std::time::Duration::from_secs(10) {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_statusline_cache(data: &WidgetData) {
    let Some(path) = dirs::home_dir().map(|h| h.join(".cache/crux/statusline-cache.json")) else {
        return;
    };
    if let Ok(json) = serde_json::to_string(data) {
        let _ = std::fs::write(path, json);
    }
}

fn fmt_cost(v: f64) -> String {
    if v < 0.01 { "$0".into() }
    else if v < 1.0 { format!("${:.2}", v) }
    else if v < 10.0 { format!("${:.1}", v) }
    else { format!("${:.0}", v) }
}

fn build_bar(pct: f64) -> String {
    let filled = ((pct / 100.0) * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    let empty = BAR_WIDTH - filled;

    let color = if pct < 40.0 { GREEN }
        else if pct < 70.0 { YELLOW }
        else { RED };

    format!(
        "{color}{}{RESET}{DIM}{}{RESET}",
        "\u{2593}".repeat(filled),
        "\u{2591}".repeat(empty),
    )
}

pub fn run_statusline(config: &Config) -> anyhow::Result<()> {
    let stdin_input = std::io::read_to_string(std::io::stdin()).unwrap_or_default();
    let input: Option<CcSessionInput> = if stdin_input.trim().is_empty() {
        None
    } else {
        serde_json::from_str(&stdin_input).ok()
    };

    let widget = read_widget_cache()
        .or_else(read_statusline_cache)
        .or_else(|| {
            let store = crate::load_store(config).ok()?;
            let data = crate::cli::widget::build_widget_data(&store, config);
            write_statusline_cache(&data);
            Some(data)
        });

    let ctx_pct = input.as_ref()
        .and_then(|i| i.context_window.as_ref())
        .and_then(|cw| cw.used_percentage)
        .unwrap_or(0.0);

    let session_id = input.as_ref().and_then(|i| i.session_id.as_deref());

    let current = session_id.and_then(|sid| {
        widget.as_ref()?.active_sessions.iter().find(|s| s.session_id == sid)
    });

    // Bar
    let bar = build_bar(ctx_pct);

    // Compressions (only when > 0)
    let compactions = current.map(|s| s.compaction_count).unwrap_or(0);
    let compress_str = if compactions > 0 {
        let color = if compactions >= 3 { RED } else { YELLOW };
        format!(" {color}{compactions}x{RESET}")
    } else {
        String::new()
    };

    // Session cost
    let session_cost = input.as_ref()
        .and_then(|i| i.cost.as_ref())
        .and_then(|c| c.total_cost_usd)
        .unwrap_or(0.0);

    // Cache warning (only when bad)
    let cache_str = current
        .filter(|s| s.cache_hit_rate < 0.60)
        .map(|s| {
            let c = s.cache_hit_rate * 100.0;
            let color = if c < 40.0 { RED } else { YELLOW };
            format!("  {color}{c:.0}% cached{RESET}")
        })
        .unwrap_or_default();

    // Today total (only when there's more than this session)
    let today_cost = widget.as_ref().map(|w| w.today.total_cost).unwrap_or(0.0);
    let global_str = if today_cost > session_cost + 0.10 {
        format!("  {DIM}\u{2502}  {} today{RESET}", fmt_cost(today_cost))
    } else {
        String::new()
    };

    print!(
        "{bar}{compress_str}  {}{cache_str}{global_str}",
        fmt_cost(session_cost),
    );

    Ok(())
}
