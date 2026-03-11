use crate::config::Config;
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config, scroll: usize) {
    let area = frame.area();
    let w = area.width;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(3),  // progress bar + burn rate
            Constraint::Length(1),  // divider
            Constraint::Length(6),  // token economics
            Constraint::Length(1),  // divider
            Constraint::Length(5),  // period summary
            Constraint::Length(1),  // divider
            Constraint::Length(1),  // projects header
            Constraint::Min(3),    // project list
            Constraint::Length(1),  // divider
            Constraint::Length(1),  // help bar
        ])
        .split(area);

    // ── Header ──
    let window_dur = config.rolling_window_duration();
    let window_agg = store.rolling_window(window_dur);
    let all = store.all_time();

    // Budget-aware progress
    let today_cost = store.today().cost;
    let (pct, budget_label) = if let Some(budget) = config.budget_daily {
        let p = (today_cost / budget * 100.0).min(150.0);
        (p, format!("  {}/{} daily budget", pricing::format_cost(today_cost), pricing::format_cost(budget)))
    } else if let Some(budget) = config.budget_weekly {
        let week_cost = store.this_week().cost;
        let p = (week_cost / budget * 100.0).min(150.0);
        (p, format!("  {}/{} weekly budget", pricing::format_cost(week_cost), pricing::format_cost(budget)))
    } else {
        let peak = estimate_peak(store, window_dur);
        let total = window_agg.total_tokens();
        let p = if peak > 0 { (total as f64 / peak as f64 * 100.0).min(100.0) } else { 0.0 };
        (p, String::new())
    };

    let header = vec![
        Line::from(vec![
            Span::styled("   usagetracker", Style::default().fg(ACCENT).bold()),
            Span::styled(
                format!("{}rolling {} window", " ".repeat((w as usize).saturating_sub(50)), config.rolling_window),
                Style::default().fg(FG_MUTED),
            ),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled(format!("   {}", pricing::format_cost(window_agg.cost)), Style::default().fg(FG).bold()),
            Span::styled("  api equivalent", Style::default().fg(FG_MUTED)),
            Span::styled(
                format!("      {} total tokens", compact(window_agg.total_tokens())),
                Style::default().fg(FG_MUTED),
            ),
            if !budget_label.is_empty() {
                Span::styled(budget_label, Style::default().fg(YELLOW))
            } else {
                Span::raw("")
            },
        ]),
    ];
    frame.render_widget(Paragraph::new(header), chunks[0]);

    // ── Progress bar + burn rate ──
    let bar_width = (w as usize).saturating_sub(8);
    let (filled, empty) = progress_bar(pct, bar_width);
    let bar_color = if pct > 90.0 { RED } else if pct > 70.0 { YELLOW } else { ACCENT };

    let cost_rate = store.cost_rate(window_dur);
    let projected_daily = cost_rate * 24.0;

    let bar_lines = vec![
        Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(filled, Style::default().fg(bar_color)),
            Span::styled(empty, Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled(format!("   {:.0}%", pct), Style::default().fg(bar_color).bold()),
            Span::styled(
                format!("      ~{}/hr", pricing::format_cost(cost_rate)),
                Style::default().fg(FG_MUTED),
            ),
            Span::styled(
                format!("      ~{}/day projected", pricing::format_cost(projected_daily)),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(bar_lines), chunks[1]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── Token economics ──
    let total_in = window_agg.input_tokens;
    let total_out = window_agg.output_tokens;
    let cache_w = window_agg.cache_creation_tokens;
    let cache_r = window_agg.cache_read_tokens;
    let total_all = window_agg.total_tokens().max(1);

    // Ratios
    let out_in_ratio = if total_in > 0 { total_out as f64 / total_in as f64 } else { 0.0 };
    let cache_hit = if (cache_r + total_in) > 0 { cache_r as f64 / (cache_r + total_in) as f64 } else { 0.0 };
    let cost_per_out_1k = if total_out > 0 { window_agg.cost / (total_out as f64 / 1000.0) } else { 0.0 };

    // Percentage of total
    let in_pct = total_in as f64 / total_all as f64 * 100.0;
    let out_pct = total_out as f64 / total_all as f64 * 100.0;
    let cw_pct = cache_w as f64 / total_all as f64 * 100.0;
    let cr_pct = cache_r as f64 / total_all as f64 * 100.0;

    let cache_color = if cache_hit > 0.6 {
        Color::Rgb(120, 190, 120)
    } else if cache_hit > 0.3 {
        YELLOW
    } else {
        FG_MUTED
    };

    let economics = vec![
        Line::from(vec![
            Span::styled("   input       ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>10}", compact(total_in)), Style::default().fg(FG)),
            Span::styled(format!("  {:>4.0}%", in_pct), Style::default().fg(FG_FAINT)),
            Span::styled(
                format!("      output/input ratio  "),
                Style::default().fg(FG_MUTED),
            ),
            Span::styled(format!("{:.2}x", out_in_ratio), Style::default().fg(FG).bold()),
        ]),
        Line::from(vec![
            Span::styled("   output      ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>10}", compact(total_out)), Style::default().fg(FG)),
            Span::styled(format!("  {:>4.0}%", out_pct), Style::default().fg(FG_FAINT)),
            Span::styled(
                format!("      cost/1K output      "),
                Style::default().fg(FG_MUTED),
            ),
            Span::styled(format!("{}", pricing::format_cost(cost_per_out_1k)), Style::default().fg(ACCENT).bold()),
        ]),
        Line::from(vec![
            Span::styled("   cache write ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>10}", compact(cache_w)), Style::default().fg(FG)),
            Span::styled(format!("  {:>4.0}%", cw_pct), Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("   cache read  ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>10}", compact(cache_r)), Style::default().fg(FG)),
            Span::styled(format!("  {:>4.0}%", cr_pct), Style::default().fg(FG_FAINT)),
            Span::styled("      cache hit rate      ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:.0}%", cache_hit * 100.0), Style::default().fg(cache_color).bold()),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("   avg/session ", Style::default().fg(FG_MUTED)),
            Span::styled(compact(store.avg_tokens_per_session()), Style::default().fg(FG)),
            Span::styled(format!("  {}", pricing::format_cost(store.avg_cost_per_session())), Style::default().fg(FG_MUTED)),
            Span::styled(format!("      {} records  {} sessions", all.record_count, all.session_count), Style::default().fg(FG_FAINT)),
        ]),
    ];
    frame.render_widget(Paragraph::new(economics), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // ── Period summary ──
    let today = store.today();
    let yesterday = store.yesterday();
    let week = store.this_week();

    let period = vec![
        period_line("   today      ", &today, true),
        period_line("   yesterday  ", &yesterday, false),
        period_line("   this week  ", &week, false),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("   all time   ", Style::default().fg(FG_MUTED)),
            Span::styled(compact(all.total_tokens()), Style::default().fg(FG)),
            Span::styled("  tokens   ", Style::default().fg(FG_FAINT)),
            Span::styled(pricing::format_cost(all.cost), Style::default().fg(ACCENT)),
            Span::styled(format!("   {} sessions", all.session_count), Style::default().fg(FG_MUTED)),
        ]),
    ];
    frame.render_widget(Paragraph::new(period), chunks[5]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[6]);

    // ── Projects header ──
    let proj_header = Line::from(vec![
        Span::styled("   projects", Style::default().fg(ACCENT)),
        Span::styled(
            format!("{}tokens       cost   sessions    last",
                " ".repeat((w as usize).saturating_sub(62).max(2))),
            Style::default().fg(FG_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(proj_header), chunks[7]);

    // ── Project list ──
    let projects = store.by_project();
    let max_rows = chunks[8].height as usize;
    let mut project_lines: Vec<Line> = Vec::new();
    let proj_scroll = scroll.min(projects.len().saturating_sub(max_rows));
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
    // Scroll indicator for projects
    if projects.len() > max_rows {
        let remaining = projects.len().saturating_sub(proj_scroll + max_rows);
        if remaining > 0 {
            if let Some(last) = project_lines.last_mut() {
                *last = Line::from(vec![
                    Span::styled(
                        format!("   ... {} more (use \u{2191}\u{2193} to scroll)", remaining),
                        Style::default().fg(FG_FAINT),
                    ),
                ]);
            }
        }
    }
    frame.render_widget(Paragraph::new(project_lines), chunks[8]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[9]);

    // ── Help bar ──
    let help = Line::from(vec![
        Span::styled("   \u{2191}\u{2193}", Style::default().fg(ACCENT)),
        Span::styled(" scroll   ", Style::default().fg(FG_MUTED)),
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
    frame.render_widget(Paragraph::new(help), chunks[10]);
}

fn period_line<'a>(label: &'a str, agg: &crate::store::Aggregation, highlight: bool) -> Line<'a> {
    let fg = if highlight { FG } else { FG_MUTED };
    let cost_fg = if highlight { ACCENT } else { FG_FAINT };
    let in_tok = agg.input_tokens;
    let out_tok = agg.output_tokens;
    let ratio = if in_tok > 0 { out_tok as f64 / in_tok as f64 } else { 0.0 };

    Line::from(vec![
        Span::styled(label.to_string(), Style::default().fg(fg)),
        Span::styled(format!("{:>10}", compact(agg.total_tokens())), Style::default().fg(fg)),
        Span::styled(format!("  {}", pricing::format_cost(agg.cost)), Style::default().fg(cost_fg)),
        Span::styled(format!("   {:.2}x o/i", ratio), Style::default().fg(FG_FAINT)),
        Span::styled(format!("   {} sess", agg.session_count), Style::default().fg(FG_MUTED)),
    ])
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

fn estimate_peak(store: &Store, window: chrono::Duration) -> u64 {
    let current = store.rolling_window(window).total_tokens();
    (current * 3 / 2).max(500_000)
}
