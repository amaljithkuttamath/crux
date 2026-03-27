use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::{Clear, Block, Borders, Paragraph};

pub fn render_help_overlay(frame: &mut ratatui::Frame) {
    let area = frame.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(FG_FAINT))
        .title(Span::styled(" Help ", Style::default().fg(ACCENT).bold()));

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let content_area = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(1)])
        .split(area)[0];

    let lines = vec![
        Line::from(vec![
            Span::styled("  Navigation", Style::default().fg(ACCENT).bold()),
            Span::styled("                     ", Style::default()),
            Span::styled("Metrics", Style::default().fg(ACCENT).bold()),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{:<34}", "\u{2500}".repeat(10)), Style::default().fg(FG_FAINT)),
            Span::styled("\u{2500}".repeat(7), Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("  \u{2191}/\u{2193} j/k  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Navigate / scroll"), Style::default().fg(FG_MUTED)),
            Span::styled("Cost     ", Style::default().fg(ACCENT)),
            Span::styled("Total spend this session", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  \u{2190}/\u{2192}      ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Drill in/out panels"), Style::default().fg(FG_MUTED)),
            Span::styled("CTX      ", Style::default().fg(ACCENT)),
            Span::styled("Context window tokens / ceiling", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Session detail view"), Style::default().fg(FG_MUTED)),
            Span::styled("Status   ", Style::default().fg(ACCENT)),
            Span::styled("Session health (see below)", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  Esc      ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Back / close"), Style::default().fg(FG_MUTED)),
            Span::styled("DUR      ", Style::default().fg(ACCENT)),
            Span::styled("Session duration", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  /        ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Search sessions"), Style::default().fg(FG_MUTED)),
            Span::styled("AGE      ", Style::default().fg(ACCENT)),
            Span::styled("Time since last activity", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("  Views + Filters", Style::default().fg(ACCENT).bold()),
            Span::styled("                    ", Style::default()),
            Span::styled("Status Values", Style::default().fg(ACCENT).bold()),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{:<34}", "\u{2500}".repeat(15)), Style::default().fg(FG_FAINT)),
            Span::styled("\u{2500}".repeat(13), Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("  b  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Browser (all sessions)"), Style::default().fg(FG_MUTED)),
            Span::styled("fresh     ", Style::default().fg(GREEN)),
            Span::styled("Low context fill", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  d  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Filter to Claude Code"), Style::default().fg(FG_MUTED)),
            Span::styled("healthy   ", Style::default().fg(GREEN)),
            Span::styled("Moderate context fill", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  c  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Filter to Cursor"), Style::default().fg(FG_MUTED)),
            Span::styled("aging     ", Style::default().fg(YELLOW)),
            Span::styled("High context fill", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  f  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Cycle source filter"), Style::default().fg(FG_MUTED)),
            Span::styled("ctx rot   ", Style::default().fg(RED)),
            Span::styled("Context window nearly full", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  s  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Stats (heatmap, history, records)"), Style::default().fg(FG_MUTED)),
            Span::styled("done      ", Style::default().fg(FG_FAINT)),
            Span::styled("Session completed", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  ?  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "This help"), Style::default().fg(FG_MUTED)),
            Span::styled("aborted   ", Style::default().fg(RED)),
            Span::styled("Session aborted (Cursor only)", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  q  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Quit"), Style::default().fg(FG_MUTED)),
        ]),
        Line::from(Span::raw("")),
        Line::from(Span::styled("  Visuals", Style::default().fg(ACCENT).bold())),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("\u{2500}".repeat(7), Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("  \u{2588}\u{2588}\u{2588}\u{2591}\u{2591}  ", Style::default().fg(GREEN)),
            Span::styled("context/budget fill    ", Style::default().fg(FG_MUTED)),
            Span::styled("green", Style::default().fg(GREEN)),
            Span::styled(" < 60%    ", Style::default().fg(FG_MUTED)),
            Span::styled("yellow", Style::default().fg(YELLOW)),
            Span::styled(" < 85%    ", Style::default().fg(FG_MUTED)),
            Span::styled("red", Style::default().fg(RED)),
            Span::styled(" > 85%", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  \u{2581}\u{2582}\u{2583}\u{2585}\u{2587}  ", Style::default().fg(ACCENT)),
            Span::styled("sparkline              height = relative value in range", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("                                        Press ", Style::default().fg(FG_FAINT)),
            Span::styled("?", Style::default().fg(ACCENT)),
            Span::styled(" or ", Style::default().fg(FG_FAINT)),
            Span::styled("Esc", Style::default().fg(ACCENT)),
            Span::styled(" to close", Style::default().fg(FG_FAINT)),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), content_area);
}
