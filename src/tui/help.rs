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
            Span::styled(format!("{:<24}", "Select session"), Style::default().fg(FG_MUTED)),
            Span::styled("Cost     ", Style::default().fg(ACCENT)),
            Span::styled("Total spend this session", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  Enter    ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Open session detail"), Style::default().fg(FG_MUTED)),
            Span::styled("CTX      ", Style::default().fg(ACCENT)),
            Span::styled("Context window tokens / ceiling", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  Tab      ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Cycle focus zones"), Style::default().fg(FG_MUTED)),
            Span::styled("Status   ", Style::default().fg(ACCENT)),
            Span::styled("Session health (see below)", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  Esc      ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Back / Close modal"), Style::default().fg(FG_MUTED)),
            Span::styled("DUR      ", Style::default().fg(ACCENT)),
            Span::styled("Session duration", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  s        ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Cycle sort column"), Style::default().fg(FG_MUTED)),
            Span::styled("AGE      ", Style::default().fg(ACCENT)),
            Span::styled("Time since last activity", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  /        ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<24}", "Search sessions"), Style::default().fg(FG_MUTED)),
            Span::styled("Savings  ", Style::default().fg(ACCENT)),
            Span::styled("Recoverable cost from shorter", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled(format!("{:<35}", ""), Style::default()),
            Span::styled("         ", Style::default()),
            Span::styled("sessions (context bloat premium)", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("  Views", Style::default().fg(ACCENT).bold()),
            Span::styled("                            ", Style::default()),
            Span::styled("Status Values", Style::default().fg(ACCENT).bold()),
        ]),
        Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{:<34}", "\u{2500}".repeat(5)), Style::default().fg(FG_FAINT)),
            Span::styled("\u{2500}".repeat(13), Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("  o  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Overview"), Style::default().fg(FG_MUTED)),
            Span::styled("fresh     ", Style::default().fg(GREEN)),
            Span::styled("Low context fill", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  d  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Claude Code sessions"), Style::default().fg(FG_MUTED)),
            Span::styled("healthy   ", Style::default().fg(GREEN)),
            Span::styled("Moderate context fill", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  c  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Cursor sessions"), Style::default().fg(FG_MUTED)),
            Span::styled("aging     ", Style::default().fg(YELLOW)),
            Span::styled("High context fill, consider refresh", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  h  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "History"), Style::default().fg(FG_MUTED)),
            Span::styled("ctx rot   ", Style::default().fg(RED)),
            Span::styled("Context window nearly full", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  ?  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "This help"), Style::default().fg(FG_MUTED)),
            Span::styled("done      ", Style::default().fg(FG_FAINT)),
            Span::styled("Session completed", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  q  ", Style::default().fg(ACCENT)),
            Span::styled(format!("{:<30}", "Quit"), Style::default().fg(FG_MUTED)),
            Span::styled("aborted   ", Style::default().fg(RED)),
            Span::styled("Session aborted (Cursor only)", Style::default().fg(FG_MUTED)),
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
            Span::styled("  \u{2593}\u{2593}\u{2593}\u{2591}\u{2591}  ", Style::default().fg(GREEN)),
            Span::styled("context mini-bar       same thresholds, compact form", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  \u{2581}\u{2582}\u{2583}\u{2585}\u{2587}  ", Style::default().fg(ACCENT)),
            Span::styled("sparkline              height = relative value in range", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  \u{25aa}\u{25aa}\u{00b7}\u{00b7}\u{25aa}  ", Style::default().fg(FG_MUTED)),
            Span::styled("activity density       block = active    dot = idle gap", Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  \u{2588}\u{2588}\u{2588}\u{2588}  ", Style::default().fg(ACCENT)),
            Span::styled("segmented bar          length = proportion of total", Style::default().fg(FG_MUTED)),
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
