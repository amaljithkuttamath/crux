use crate::config::Config;
use crate::store::Store;
use super::widgets::{self, ACCENT, MUTED};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut ratatui::Frame, store: &Store, _config: &Config) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(2),  // title
            Constraint::Length(1),  // spacer
            Constraint::Min(5),    // bar chart
            Constraint::Length(1),  // spacer
            Constraint::Length(3),  // model split
            Constraint::Length(2),  // help
        ])
        .split(area);

    let title = Paragraph::new("  trends (last 14 days)")
        .style(Style::default().fg(ACCENT).bold());
    frame.render_widget(title, chunks[0]);

    // Bar chart
    let days = store.by_day(14);
    let max_total = days
        .iter()
        .map(|d| d.input_tokens + d.output_tokens + d.cache_creation_tokens + d.cache_read_tokens)
        .max()
        .unwrap_or(1);

    let bar_width = (chunks[2].width).saturating_sub(20);
    let mut lines: Vec<Line> = Vec::new();

    for day in days.iter().rev() {
        let total =
            day.input_tokens + day.output_tokens + day.cache_creation_tokens + day.cache_read_tokens;
        let weekday = day.date.format("%a").to_string();
        lines.push(widgets::bar_chart_row(&weekday, total, max_total, bar_width));
    }

    let chart = Paragraph::new(lines);
    frame.render_widget(chart, chunks[2]);

    // Model split (placeholder, would need model tracking in aggregation)
    let model_info = Paragraph::new(Line::from(vec![
        Span::styled("  by model: ", Style::default().fg(MUTED)),
        Span::styled("(coming soon)", Style::default().fg(MUTED)),
    ]));
    frame.render_widget(model_info, chunks[4]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("  esc", Style::default().fg(ACCENT)),
        Span::styled(" back    ", Style::default().fg(MUTED)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(MUTED)),
    ]));
    frame.render_widget(help, chunks[5]);
}
