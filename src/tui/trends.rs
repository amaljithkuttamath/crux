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
            Constraint::Length(1),  // spacer
            Constraint::Min(5),    // bar chart
            Constraint::Length(1),  // divider
            Constraint::Length(1),  // model header
            Constraint::Length(4),  // model breakdown
            Constraint::Length(1),  // divider
            Constraint::Length(1),  // help
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled("   usage trends", Style::default().fg(ACCENT).bold()),
        Span::styled("   last 14 days", Style::default().fg(FG_FAINT)),
    ]));
    frame.render_widget(title, chunks[0]);

    // ── Bar chart ──
    let days = store.by_day(14);
    let max_total = days
        .iter()
        .map(|d| d.input_tokens + d.output_tokens + d.cache_creation_tokens + d.cache_read_tokens)
        .max()
        .unwrap_or(1);

    let bar_width = w.saturating_sub(30);
    let max_rows = chunks[2].height as usize;
    let mut lines: Vec<Line> = Vec::new();

    for day in days.iter().take(max_rows).rev() {
        let total = day.input_tokens + day.output_tokens + day.cache_creation_tokens + day.cache_read_tokens;
        let weekday = day.date.format("%a").to_string();
        let date_str = day.date.format("%m/%d").to_string();
        lines.push(trend_bar(&weekday, &date_str, total, max_total, bar_width));
    }

    frame.render_widget(Paragraph::new(lines), chunks[2]);

    // Divider
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // ── Model breakdown ──
    let model_header = Line::from(vec![
        Span::styled("   model breakdown", Style::default().fg(ACCENT)),
        Span::styled(
            format!("{}requests     tokens        cost", " ".repeat((w as usize).saturating_sub(56).max(2))),
            Style::default().fg(FG_FAINT),
        ),
    ]);
    frame.render_widget(Paragraph::new(model_header), chunks[4]);

    let models = store.by_model();
    let total_records: usize = models.iter().map(|m| m.record_count).sum();
    let mut model_lines: Vec<Line> = Vec::new();

    for m in models.iter().take(4) {
        let pct = if total_records > 0 { m.record_count * 100 / total_records } else { 0 };
        let total_tok = m.input_tokens + m.output_tokens;
        model_lines.push(Line::from(vec![
            Span::styled(format!("   {:<12}", m.name), Style::default().fg(FG)),
            Span::styled(format!("{:>3}%", pct), Style::default().fg(FG_MUTED)),
            Span::styled(
                format!("{}  {:>8}", " ".repeat((w as usize).saturating_sub(56).max(2)), m.record_count),
                Style::default().fg(FG_FAINT),
            ),
            Span::styled(format!("  {:>10}", compact(total_tok)), Style::default().fg(FG_MUTED)),
            Span::styled(format!("    {:>10}", pricing::format_cost(m.cost)), Style::default().fg(FG_FAINT)),
        ]));
    }

    frame.render_widget(Paragraph::new(model_lines), chunks[5]);

    // Divider
    frame.render_widget(Paragraph::new(divider(w)), chunks[6]);

    let help = Line::from(vec![
        Span::styled("   esc", Style::default().fg(ACCENT)),
        Span::styled(" back   ", Style::default().fg(FG_FAINT)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(FG_FAINT)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[7]);
}
