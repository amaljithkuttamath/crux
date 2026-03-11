use crate::config::Config;
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut ratatui::Frame, store: &Store, _config: &Config) {
    let area = frame.area();
    let w = area.width;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),  // title
            Constraint::Length(1),  // header
            Constraint::Length(1),  // divider
            Constraint::Min(5),    // rows
            Constraint::Length(1),  // divider
            Constraint::Length(1),  // help
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled("   daily breakdown", Style::default().fg(ACCENT).bold()),
    ]));
    frame.render_widget(title, chunks[0]);

    let header = Line::from(vec![
        Span::styled(format!("   {:<12}", "date"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>10}", "input"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>10}", "output"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>12}", "cache"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>10}", "sessions"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>10}", "cost"), Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[1]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    let days = store.by_day(30);
    let today = chrono::Utc::now().date_naive();
    let max_rows = chunks[3].height as usize;
    let mut lines: Vec<Line> = Vec::new();

    for day in days.iter().take(max_rows) {
        let is_today = day.date == today;
        let fg = if is_today { FG } else { FG_MUTED };
        let cost_fg = if is_today { ACCENT } else { FG_MUTED };
        let cache = day.cache_creation_tokens + day.cache_read_tokens;

        lines.push(Line::from(vec![
            Span::styled(
                format!("   {:<12}", if is_today { "today".to_string() } else { day.date.format("%b %d").to_string() }),
                Style::default().fg(fg),
            ),
            Span::styled(format!("{:>10}", compact(day.input_tokens)), Style::default().fg(fg)),
            Span::styled(format!("{:>10}", compact(day.output_tokens)), Style::default().fg(fg)),
            Span::styled(format!("{:>12}", compact(cache)), Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>10}", day.session_count), Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>10}", pricing::format_cost(day.cost)), Style::default().fg(cost_fg)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    let help = Line::from(vec![
        Span::styled("   esc", Style::default().fg(ACCENT)),
        Span::styled(" back   ", Style::default().fg(FG_FAINT)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(FG_FAINT)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[5]);
}
