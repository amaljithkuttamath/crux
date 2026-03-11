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
            Constraint::Min(4),    // model list
            Constraint::Length(1),  // divider
            Constraint::Length(3),  // pricing reference
            Constraint::Length(1),  // divider
            Constraint::Length(1),  // help
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled("   model usage", Style::default().fg(ACCENT).bold()),
        Span::styled("   api-equivalent pricing", Style::default().fg(FG_FAINT)),
    ]));
    frame.render_widget(title, chunks[0]);

    let header = Line::from(vec![
        Span::styled(format!("   {:<12}", "model"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>10}", "requests"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>12}", "input tok"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>12}", "output tok"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>10}", "cost"), Style::default().fg(FG_MUTED)),
        Span::styled(format!("{:>8}", "share"), Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[1]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    let models = store.by_model();
    let total_cost: f64 = models.iter().map(|m| m.cost).sum();
    let max_rows = chunks[3].height as usize;
    let mut lines: Vec<Line> = Vec::new();

    for m in models.iter().take(max_rows) {
        let cost_pct = if total_cost > 0.0 { (m.cost / total_cost * 100.0) as u64 } else { 0 };
        let bar_w = 6usize;
        let filled = (cost_pct as usize * bar_w / 100).min(bar_w);
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_w - filled);

        lines.push(Line::from(vec![
            Span::styled(format!("   {:<12}", m.name), Style::default().fg(FG)),
            Span::styled(format!("{:>10}", m.record_count), Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>12}", compact(m.input_tokens)), Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>12}", compact(m.output_tokens)), Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>10}", pricing::format_cost(m.cost)), Style::default().fg(ACCENT)),
            Span::styled(format!("  {}", bar), Style::default().fg(ACCENT_DIM)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // Pricing reference
    let pricing_ref = vec![
        Line::from(vec![
            Span::styled("   pricing (per 1M tokens):", Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("   sonnet ", Style::default().fg(FG_MUTED)),
            Span::styled("$3/in $15/out", Style::default().fg(FG_FAINT)),
            Span::styled("      opus ", Style::default().fg(FG_MUTED)),
            Span::styled("$15/in $75/out", Style::default().fg(FG_FAINT)),
            Span::styled("      haiku ", Style::default().fg(FG_MUTED)),
            Span::styled("$0.80/in $4/out", Style::default().fg(FG_FAINT)),
        ]),
    ];
    frame.render_widget(Paragraph::new(pricing_ref), chunks[5]);

    frame.render_widget(Paragraph::new(divider(w)), chunks[6]);

    let help = Line::from(vec![
        Span::styled("   esc", Style::default().fg(ACCENT)),
        Span::styled(" back   ", Style::default().fg(FG_FAINT)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(FG_FAINT)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[7]);
}
