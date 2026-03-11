use crate::config::Config;
use crate::store::Store;
use super::widgets::{self, ACCENT, MUTED};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(2),  // title
            Constraint::Length(3),  // rolling window bar
            Constraint::Length(2),  // token stats
            Constraint::Length(1),  // burn rate
            Constraint::Length(1),  // spacer
            Constraint::Length(5),  // period summary
            Constraint::Length(1),  // spacer
            Constraint::Length(1),  // projects header
            Constraint::Min(3),    // project list
            Constraint::Length(2),  // help bar
        ])
        .split(area);

    // Title
    let title = Paragraph::new("  usagetracker")
        .style(Style::default().fg(ACCENT).bold());
    frame.render_widget(title, chunks[0]);

    // Rolling window
    let window_dur = config.rolling_window_duration();
    let window = store.rolling_window(window_dur);
    let total_window = window.total_tokens();

    // Estimate percentage based on a reasonable daily max
    // For Max 5, typical heavy usage is ~500K tokens in 5h
    let baseline = estimate_baseline(store, window_dur);
    let pct = if baseline > 0 {
        (total_window as f64 / baseline as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let window_label = format!(
        "  {} window                            {:.0}% used",
        config.rolling_window, pct
    );
    let bar = widgets::rolling_window_bar(pct, &window_label);
    frame.render_widget(bar, chunks[1]);

    // Token stats
    let stats = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("    {}in", widgets::compact(window.input_tokens)),
            Style::default().fg(Color::White),
        ),
        Span::raw("    "),
        Span::styled(
            format!("{}out", widgets::compact(window.output_tokens)),
            Style::default().fg(Color::White),
        ),
        Span::raw("    "),
        Span::styled(
            format!(
                "{}cached",
                widgets::compact(window.cache_creation_tokens + window.cache_read_tokens)
            ),
            Style::default().fg(MUTED),
        ),
    ]));
    frame.render_widget(stats, chunks[2]);

    // Burn rate
    let rate = store.burn_rate(window_dur);
    let burn = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("    ~{}/hr", widgets::compact(rate as u64)),
            Style::default().fg(MUTED),
        ),
    ]));
    frame.render_widget(burn, chunks[3]);

    // Period summary
    let today = store.today();
    let yesterday = store.yesterday();
    let week = store.this_week();

    let period_lines = vec![
        format_period_line("  today", &today),
        format_period_line("  yesterday", &yesterday),
        format_period_line("  this week", &week),
    ];
    let period = Paragraph::new(period_lines);
    frame.render_widget(period, chunks[5]);

    // Projects header
    let header = Paragraph::new(Span::styled("  projects", Style::default().fg(ACCENT)));
    frame.render_widget(header, chunks[7]);

    // Project list
    let projects = store.by_project();
    let max_rows = chunks[8].height as usize;
    let mut project_lines: Vec<Line> = Vec::new();
    for p in projects.iter().take(max_rows) {
        let total =
            p.input_tokens + p.output_tokens + p.cache_creation_tokens + p.cache_read_tokens;
        project_lines.push(Line::from(vec![
            Span::styled(
                format!("    {:<24}", truncate(&p.name, 24)),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{:>8}", widgets::compact(total)),
                Style::default().fg(MUTED),
            ),
            Span::styled(
                format!("    {} sessions", p.session_count),
                Style::default().fg(MUTED),
            ),
            Span::styled(
                format!("    {}", widgets::format_ago(p.last_used)),
                Style::default().fg(MUTED),
            ),
        ]));
    }
    let project_widget = Paragraph::new(project_lines);
    frame.render_widget(project_widget, chunks[8]);

    // Help bar
    let help = Paragraph::new(Line::from(vec![
        Span::styled("  q", Style::default().fg(ACCENT)),
        Span::styled(" quit    ", Style::default().fg(MUTED)),
        Span::styled("d", Style::default().fg(ACCENT)),
        Span::styled(" daily    ", Style::default().fg(MUTED)),
        Span::styled("t", Style::default().fg(ACCENT)),
        Span::styled(" trends", Style::default().fg(MUTED)),
    ]));
    frame.render_widget(help, chunks[9]);
}

fn format_period_line<'a>(
    label: &'a str,
    agg: &crate::store::Aggregation,
) -> Line<'a> {
    let total = agg.total_tokens();
    Line::from(vec![
        Span::styled(format!("{:<14}", label), Style::default().fg(Color::White)),
        Span::styled(
            format!(
                "{:>3} sessions          {} tokens",
                agg.session_count,
                widgets::compact(total)
            ),
            Style::default().fg(MUTED),
        ),
    ])
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max - 3])
    } else {
        s.to_string()
    }
}

fn estimate_baseline(store: &Store, window: chrono::Duration) -> u64 {
    // Use the max observed usage in any window of this size as the baseline
    // For now, use a simple heuristic: 2x the current window usage, min 100K
    let current = store.rolling_window(window).total_tokens();
    (current * 2).max(100_000)
}
