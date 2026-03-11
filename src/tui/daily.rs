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
            Constraint::Length(2),  // header
            Constraint::Min(5),    // rows
            Constraint::Length(2),  // help
        ])
        .split(area);

    let title = Paragraph::new("  daily")
        .style(Style::default().fg(ACCENT).bold());
    frame.render_widget(title, chunks[0]);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(format!("  {:<12}", "Date"), Style::default().fg(MUTED)),
        Span::styled(format!("{:>10}", "Input"), Style::default().fg(MUTED)),
        Span::styled(format!("{:>10}", "Output"), Style::default().fg(MUTED)),
        Span::styled(format!("{:>12}", "Cache"), Style::default().fg(MUTED)),
        Span::styled(format!("{:>10}", "Sessions"), Style::default().fg(MUTED)),
    ]));
    frame.render_widget(header, chunks[1]);

    let days = store.by_day(30);
    let today = chrono::Utc::now().date_naive();
    let max_rows = chunks[2].height as usize;
    let mut lines: Vec<Line> = Vec::new();

    for day in days.iter().take(max_rows) {
        let is_today = day.date == today;
        let fg = if is_today { ACCENT } else { Color::White };
        let cache = day.cache_creation_tokens + day.cache_read_tokens;

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<12}", day.date), Style::default().fg(fg)),
            Span::styled(
                format!("{:>10}", widgets::compact(day.input_tokens)),
                Style::default().fg(fg),
            ),
            Span::styled(
                format!("{:>10}", widgets::compact(day.output_tokens)),
                Style::default().fg(fg),
            ),
            Span::styled(
                format!("{:>12}", widgets::compact(cache)),
                Style::default().fg(MUTED),
            ),
            Span::styled(
                format!("{:>10}", day.session_count),
                Style::default().fg(MUTED),
            ),
        ]));
    }

    let table = Paragraph::new(lines);
    frame.render_widget(table, chunks[2]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("  esc", Style::default().fg(ACCENT)),
        Span::styled(" back    ", Style::default().fg(MUTED)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(MUTED)),
    ]));
    frame.render_widget(help, chunks[3]);
}
