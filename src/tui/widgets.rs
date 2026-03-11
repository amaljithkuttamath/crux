use ratatui::prelude::*;

// Warm palette, high contrast
pub const ACCENT: Color = Color::Rgb(210, 140, 90);    // bright copper
pub const ACCENT_DIM: Color = Color::Rgb(160, 110, 70); // soft copper
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

/// Horizontal bar for trends, returns owned Line
pub fn trend_bar(label: &str, date_str: &str, value: u64, max_value: u64, width: u16) -> Line<'static> {
    let bar_width = width as usize;
    let filled = if max_value > 0 {
        ((value as f64 / max_value as f64) * bar_width as f64).round() as usize
    } else {
        0
    };
    let bar: String = "█".repeat(filled);
    let empty: String = "░".repeat(bar_width.saturating_sub(filled));

    Line::from(vec![
        Span::styled(format!("    {:<4}", label), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:<12}", date_str), Style::default().fg(FG_MUTED)),
        Span::styled(bar, Style::default().fg(ACCENT)),
        Span::styled(empty, Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {}", compact(value)), Style::default().fg(FG_MUTED)),
    ])
}

/// Section divider line
pub fn divider(width: u16) -> Line<'static> {
    let line: String = "─".repeat(width.saturating_sub(6) as usize);
    Line::from(Span::styled(format!("   {}", line), Style::default().fg(FG_FAINT)))
}

