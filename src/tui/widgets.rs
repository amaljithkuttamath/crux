use ratatui::prelude::*;
use ratatui::widgets::*;

pub const ACCENT: Color = Color::Rgb(184, 122, 80);
pub const MUTED: Color = Color::DarkGray;
pub const DIM: Color = Color::Rgb(80, 80, 80);

pub fn rolling_window_bar<'a>(percentage: f64, label: &'a str) -> Gauge<'a> {
    let color = if percentage > 90.0 {
        Color::Red
    } else if percentage > 70.0 {
        Color::Yellow
    } else {
        ACCENT
    };

    Gauge::default()
        .gauge_style(Style::default().fg(color).bg(DIM))
        .percent(percentage.min(100.0) as u16)
        .label(Span::styled(label, Style::default().fg(Color::White)))
}

pub fn compact(n: u64) -> String {
    if n >= 1_000_000 {
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

pub fn bar_chart_row(label: &str, value: u64, max_value: u64, width: u16) -> Line<'static> {
    let bar_width = if max_value > 0 {
        ((value as f64 / max_value as f64) * width as f64) as usize
    } else {
        0
    };
    let bar: String = "━".repeat(bar_width);
    let empty: String = " ".repeat(width as usize - bar_width);

    Line::from(vec![
        Span::styled(format!("  {:<6}", label), Style::default().fg(Color::White)),
        Span::styled(bar, Style::default().fg(ACCENT)),
        Span::styled(empty, Style::default()),
        Span::styled(format!(" {}", compact(value)), Style::default().fg(MUTED)),
    ])
}
