use ratatui::prelude::*;

// ── Warm mineral palette ──
// Inspired by sandstone, copper ore, and desert sky at dusk.
// High contrast ratios against dark terminals, cohesive warmth.
pub const ACCENT: Color = Color::Rgb(224, 155, 95);     // warm amber (headers, highlights, keys)
pub const ACCENT2: Color = Color::Rgb(140, 180, 160);   // sage green (secondary accent, CC badge)
pub const FG: Color = Color::Rgb(240, 234, 226);        // warm white (primary text)
pub const FG_MUTED: Color = Color::Rgb(175, 168, 158);  // sandstone gray (secondary text)
pub const FG_FAINT: Color = Color::Rgb(105, 100, 92);   // deep clay (borders, labels)
pub const GREEN: Color = Color::Rgb(130, 195, 130);     // healthy green
pub const YELLOW: Color = Color::Rgb(235, 195, 85);     // desert amber (warnings)
pub const RED: Color = Color::Rgb(225, 95, 85);         // terracotta (critical)
pub const BLUE: Color = Color::Rgb(120, 160, 210);      // sky blue (Cursor badge, charts)
pub const PURPLE: Color = Color::Rgb(170, 140, 200);    // dusk violet (model highlights)

pub fn compact(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn format_ago(time: chrono::DateTime<chrono::Utc>) -> String {
    let diff = chrono::Utc::now() - time;
    if diff.num_days() > 0 {
        format!("{}d ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{}h ago", diff.num_hours())
    } else {
        format!("{}m ago", diff.num_minutes().max(1))
    }
}

/// Section divider line
pub fn divider(width: u16) -> Line<'static> {
    let line: String = "\u{2500}".repeat(width.saturating_sub(6) as usize);
    Line::from(Span::styled(format!("   {}", line), Style::default().fg(FG_FAINT)))
}

/// Sparkline from float values using Unicode block characters
pub fn spark(values: &[f64]) -> String {
    let blocks = ['_', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];
    let max = values.iter().cloned().fold(0.0f64, f64::max);
    if max <= 0.0 {
        return "_".repeat(values.len());
    }
    values.iter().map(|v| {
        let idx = ((v / max) * 8.0).round() as usize;
        blocks[idx.min(8)]
    }).collect()
}

/// Horizontal bar with sub-character precision using Unicode 8th blocks.
/// Returns (filled_string, empty_string) for styled rendering.
pub fn smooth_bar(value: f64, max: f64, width: usize) -> (String, String) {
    if max <= 0.0 || width == 0 {
        return (String::new(), "\u{2591}".repeat(width));
    }
    let ratio = (value / max).clamp(0.0, 1.0);
    let total_eighths = (ratio * width as f64 * 8.0).round() as usize;
    let full_blocks = total_eighths / 8;
    let remainder = total_eighths % 8;

    let partials = [' ', '\u{258F}', '\u{258E}', '\u{258D}', '\u{258C}', '\u{258B}', '\u{258A}', '\u{2589}'];

    let mut filled = "\u{2588}".repeat(full_blocks);
    let empty_start = if remainder > 0 && full_blocks < width {
        filled.push(partials[remainder]);
        full_blocks + 1
    } else {
        full_blocks
    };
    let empty = "\u{2591}".repeat(width.saturating_sub(empty_start));
    (filled, empty)
}

/// Shared help bar from key-label pairs
pub fn help_bar(bindings: &[(&str, &str)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![Span::styled("   ", Style::default())];
    for (key, label) in bindings {
        spans.push(Span::styled(key.to_string(), Style::default().fg(ACCENT)));
        spans.push(Span::styled(format!(" {}  ", label), Style::default().fg(FG_MUTED)));
    }
    Line::from(spans)
}

/// Truncate a string to max width, first line only
pub fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > max {
        format!("{}...", &first_line[..max.saturating_sub(3)])
    } else {
        first_line.to_string()
    }
}

/// Truncate model name for compact display
pub fn truncate_model(model: &str, max: usize) -> String {
    let clean = model
        .replace("claude-", "")
        .replace("anthropic/", "")
        .replace("openai/", "")
        .replace("google/", "");
    if clean.len() > max {
        format!("{}...", &clean[..max.saturating_sub(3)])
    } else {
        clean
    }
}

/// Shorten tool names for compact display
pub fn shorten_tool(name: &str) -> String {
    match name {
        "Read" => "Rd".to_string(),
        "Write" => "Wr".to_string(),
        "Edit" => "Ed".to_string(),
        "Bash" => "Sh".to_string(),
        "Glob" => "Gl".to_string(),
        "Grep" => "Gr".to_string(),
        "Agent" => "Ag".to_string(),
        "Skill" => "Sk".to_string(),
        "WebFetch" => "WF".to_string(),
        "WebSearch" => "WS".to_string(),
        "NotebookEdit" => "NE".to_string(),
        _ => {
            if name.len() > 4 {
                name[..4].to_string()
            } else {
                name.to_string()
            }
        }
    }
}

pub fn month_abbrev(month: u32) -> &'static str {
    match month {
        1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR",
        5 => "MAY", 6 => "JUN", 7 => "JUL", 8 => "AUG",
        9 => "SEP", 10 => "OCT", 11 => "NOV", 12 => "DEC",
        _ => "???",
    }
}

/// Clean project name for display
pub fn display_project_name(raw: &str) -> String {
    let mut name = raw.to_string();
    // Strip common prefixes
    for prefix in &["lab-", "portfolio-", "site-", "archive-"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            name = rest.to_string();
        }
    }
    // Clean up user path artifacts
    if name.contains("-Users-") {
        if let Some(idx) = name.find("-Developer-") {
            name = name[idx + 11..].to_string();
        } else if name.ends_with("-Developer") {
            name = "Developer".to_string();
        }
    }
    if name == "-private-tmp" || name == "private-tmp" {
        name = "tmp".to_string();
    }
    name
}
