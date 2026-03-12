use ratatui::prelude::*;

// Warm palette, high contrast
pub const ACCENT: Color = Color::Rgb(210, 140, 90);    // bright copper
pub const FG: Color = Color::Rgb(235, 230, 224);        // warm white
pub const FG_MUTED: Color = Color::Rgb(170, 164, 155);  // readable gray
pub const FG_FAINT: Color = Color::Rgb(110, 105, 98);   // subtle but visible
pub const YELLOW: Color = Color::Rgb(230, 190, 90);      // warning
pub const RED: Color = Color::Rgb(220, 100, 90);          // critical

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
