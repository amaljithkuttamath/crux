use crate::config::Config;
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config, scroll: usize) {
    let area = frame.area();
    let w = area.width;

    let active = store.active_sessions(24);
    let active_count = active.len();

    // Dynamic layout: active sessions get more space when present
    let active_height = if active_count > 0 {
        (active_count as u16 * 3 + 2).min(12)
    } else {
        2
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),              // title
            Constraint::Length(active_height),   // active sessions
            Constraint::Length(1),              // divider
            Constraint::Length(5),              // today + cost rate
            Constraint::Length(1),              // divider
            Constraint::Length(1),              // projects header
            Constraint::Min(3),                // project list
            Constraint::Length(1),              // divider
            Constraint::Length(1),              // help bar
        ])
        .split(area);

    // ── Title ──
    let cost_rate = store.cost_rate(config.rolling_window_duration());
    let today_cost = store.today().cost;
    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("   usagetracker", Style::default().fg(ACCENT).bold()),
            Span::styled(
                format!("{}~{}/hr  {} today",
                    " ".repeat((w as usize).saturating_sub(50)),
                    pricing::format_cost(cost_rate),
                    pricing::format_cost(today_cost),
                ),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ]);
    frame.render_widget(title, chunks[0]);

    // ── Active Sessions (hero section) ──
    if active_count > 0 {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("   LIVE", Style::default().fg(Color::Rgb(120, 190, 120)).bold()),
                Span::styled(
                    format!("  {} active session{}", active_count, if active_count > 1 { "s" } else { "" }),
                    Style::default().fg(FG_MUTED),
                ),
            ]),
        ];

        for (meta, analysis) in active.iter().take(3) {
            let cost = analysis.total_cost;
            let grade = grade_letter(analysis);
            let grade_color = match grade {
                "A" => Color::Rgb(120, 190, 120),
                "B" => ACCENT,
                "C" => YELLOW,
                _ => RED,
            };

            // Context progress bar toward 167K ceiling
            let ctx_pct = (analysis.context_current as f64 / 167_000.0 * 100.0).min(100.0);
            let bar_w = 20usize;
            let filled = ((ctx_pct / 100.0) * bar_w as f64).round() as usize;
            let bar_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { Color::Rgb(120, 190, 120) };

            let name_w = (w as usize).saturating_sub(75).max(8);
            let topic = truncate(&meta.first_message, name_w);

            // Line 1: project + topic + grade
            lines.push(Line::from(vec![
                Span::styled(format!("   {:<width$}", topic, width = name_w), Style::default().fg(FG)),
                Span::styled(format!("  {}m", meta.user_count), Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {}", pricing::format_cost(cost)), Style::default().fg(ACCENT)),
                Span::styled(format!("  {}", grade), Style::default().fg(grade_color).bold()),
                Span::styled(format!("  {}", format_ago_short(meta.end_time)), Style::default().fg(FG_FAINT)),
            ]));

            // Line 2: context bar + metrics
            let bar_filled: String = "\u{2588}".repeat(filled);
            let bar_empty: String = "\u{2591}".repeat(bar_w.saturating_sub(filled));
            lines.push(Line::from(vec![
                Span::styled("   ctx ", Style::default().fg(FG_FAINT)),
                Span::styled(bar_filled, Style::default().fg(bar_color)),
                Span::styled(bar_empty, Style::default().fg(FG_FAINT)),
                Span::styled(
                    format!(" {}  {:.1}x growth", compact(analysis.context_current), analysis.context_growth),
                    Style::default().fg(FG_MUTED),
                ),
                Span::styled(
                    format!("  cache {:.0}%", analysis.cache_hit_rate * 100.0),
                    Style::default().fg(FG_FAINT),
                ),
                if analysis.compaction_count > 0 {
                    Span::styled(
                        format!("  {} compactions", analysis.compaction_count),
                        Style::default().fg(FG_FAINT),
                    )
                } else {
                    Span::raw("")
                },
            ]));

            // Spacing between sessions
            if active.len() > 1 {
                lines.push(Line::from(Span::raw("")));
            }
        }

        frame.render_widget(Paragraph::new(lines), chunks[1]);
    } else {
        let no_active = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("   no active sessions", Style::default().fg(FG_FAINT)),
            ]),
        ]);
        frame.render_widget(no_active, chunks[1]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── Today + Period Summary ──
    let today = store.today();
    let yesterday = store.yesterday();
    let week = store.this_week();
    let all = store.all_time();

    let (budget_str, budget_pct) = if let Some(budget) = config.budget_daily {
        let p = today.cost / budget * 100.0;
        (format!("  {:.0}% of daily budget", p), Some(p))
    } else if let Some(budget) = config.budget_weekly {
        let p = week.cost / budget * 100.0;
        (format!("  {:.0}% of weekly budget", p), Some(p))
    } else {
        (String::new(), None)
    };

    let budget_color = match budget_pct {
        Some(p) if p > 90.0 => RED,
        Some(p) if p > 70.0 => YELLOW,
        _ => FG_FAINT,
    };

    let period = vec![
        period_line("   today      ", &today, true, budget_str, budget_color),
        period_line("   yesterday  ", &yesterday, false, String::new(), FG_FAINT),
        period_line("   this week  ", &week, false, String::new(), FG_FAINT),
        Line::from(vec![
            Span::styled("   all time   ", Style::default().fg(FG_MUTED)),
            Span::styled(compact(all.total_tokens()), Style::default().fg(FG_MUTED)),
            Span::styled("  tokens  ", Style::default().fg(FG_FAINT)),
            Span::styled(pricing::format_cost(all.cost), Style::default().fg(FG_MUTED)),
            Span::styled(format!("  {} sessions", all.session_count), Style::default().fg(FG_FAINT)),
        ]),
    ];
    frame.render_widget(Paragraph::new(period), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // ── Projects header ──
    let proj_header = Line::from(vec![
        Span::styled("   projects", Style::default().fg(ACCENT)),
        Span::styled(
            format!("{}tokens       cost   sessions    last",
                " ".repeat((w as usize).saturating_sub(62).max(2))),
            Style::default().fg(FG_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(proj_header), chunks[5]);

    // ── Project list ──
    let projects = store.by_project();
    let max_rows = chunks[6].height as usize;
    let proj_scroll = scroll.min(projects.len().saturating_sub(max_rows));
    let mut project_lines: Vec<Line> = Vec::new();
    for p in projects.iter().skip(proj_scroll).take(max_rows) {
        let total = p.input_tokens + p.output_tokens + p.cache_creation_tokens + p.cache_read_tokens;
        let name_w = (w as usize).saturating_sub(55).max(12);
        project_lines.push(Line::from(vec![
            Span::styled(
                format!("   {:<width$}", truncate(&p.name, name_w), width = name_w),
                Style::default().fg(FG),
            ),
            Span::styled(format!("{:>10}", compact(total)), Style::default().fg(FG_MUTED)),
            Span::styled(format!("  {:>8}", pricing::format_cost(p.cost)), Style::default().fg(FG_MUTED)),
            Span::styled(format!("   {:>5}", p.session_count), Style::default().fg(FG_MUTED)),
            Span::styled(format!("    {:>8}", format_ago(p.last_used)), Style::default().fg(FG_MUTED)),
        ]));
    }

    if projects.len() > max_rows {
        let remaining = projects.len().saturating_sub(proj_scroll + max_rows);
        if remaining > 0 {
            if let Some(last) = project_lines.last_mut() {
                *last = Line::from(vec![
                    Span::styled(
                        format!("   ... {} more", remaining),
                        Style::default().fg(FG_FAINT),
                    ),
                ]);
            }
        }
    }
    frame.render_widget(Paragraph::new(project_lines), chunks[6]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[7]);

    // ── Help bar ──
    let help = Line::from(vec![
        Span::styled("   \u{2191}\u{2193}", Style::default().fg(ACCENT)),
        Span::styled(" scroll  ", Style::default().fg(FG_MUTED)),
        Span::styled("d", Style::default().fg(ACCENT)),
        Span::styled(" daily  ", Style::default().fg(FG_MUTED)),
        Span::styled("t", Style::default().fg(ACCENT)),
        Span::styled(" trends  ", Style::default().fg(FG_MUTED)),
        Span::styled("m", Style::default().fg(ACCENT)),
        Span::styled(" models  ", Style::default().fg(FG_MUTED)),
        Span::styled("i", Style::default().fg(ACCENT)),
        Span::styled(" insights  ", Style::default().fg(FG_MUTED)),
        Span::styled("s", Style::default().fg(ACCENT)),
        Span::styled(" sessions  ", Style::default().fg(FG_MUTED)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[8]);
}

fn period_line<'a>(label: &'a str, agg: &crate::store::Aggregation, highlight: bool, extra: String, extra_color: Color) -> Line<'a> {
    let fg = if highlight { FG } else { FG_MUTED };
    let cost_fg = if highlight { ACCENT } else { FG_FAINT };

    let mut spans = vec![
        Span::styled(label.to_string(), Style::default().fg(fg)),
        Span::styled(format!("{:>10}", compact(agg.total_tokens())), Style::default().fg(fg)),
        Span::styled(format!("  {}", pricing::format_cost(agg.cost)), Style::default().fg(cost_fg)),
        Span::styled(format!("   {} sess", agg.session_count), Style::default().fg(FG_FAINT)),
    ];
    if !extra.is_empty() {
        spans.push(Span::styled(extra, Style::default().fg(extra_color)));
    }
    Line::from(spans)
}

fn grade_letter(analysis: &crate::store::SessionAnalysis) -> &'static str {
    let mut score = 100i32;
    if analysis.context_growth > 8.0 { score -= 30; }
    else if analysis.context_growth > 5.0 { score -= 20; }
    else if analysis.context_growth > 3.0 { score -= 10; }
    if analysis.output_efficiency < 0.1 { score -= 30; }
    else if analysis.output_efficiency < 0.2 { score -= 15; }
    if analysis.cost_per_1k_output > 1.0 { score -= 20; }
    else if analysis.cost_per_1k_output > 0.5 { score -= 10; }
    if analysis.compaction_count > 0 { score += 5; }
    match score {
        90..=200 => "A",
        75..=89 => "B",
        60..=74 => "C",
        40..=59 => "D",
        _ => "F",
    }
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > max {
        format!("{}...", &first_line[..max.saturating_sub(3)])
    } else {
        first_line.to_string()
    }
}

fn format_ago_short(time: chrono::DateTime<chrono::Utc>) -> String {
    let diff = chrono::Utc::now() - time;
    if diff.num_minutes() < 2 {
        "now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else {
        format!("{}d ago", diff.num_days())
    }
}

