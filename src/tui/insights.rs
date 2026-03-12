use crate::config::Config;
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config) {
    let area = frame.area();
    let w = area.width;
    let insights = store.insights_with_days(config.sparkline_days);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),   // title + sparkline
            Constraint::Length(6),   // efficiency bars
            Constraint::Length(1),   // divider
            Constraint::Length(3),   // 24h activity chart
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // heaviest sessions
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help
        ])
        .split(area);

    // ── Title ──
    let sparkline_str = spark(&insights.daily_costs);
    let total_7d: f64 = insights.daily_costs.iter().sum();
    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("   insights", Style::default().fg(ACCENT).bold()),
            Span::styled(
                format!("{}{}d spend  {}  {}",
                    " ".repeat((w as usize).saturating_sub(55)),
                    config.sparkline_days,
                    sparkline_str,
                    pricing::format_cost(total_7d),
                ),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ]);
    frame.render_widget(title, chunks[0]);

    // ── Efficiency bars ──
    let all = store.all_time();
    let cache_denom = all.cache_read_tokens + all.input_tokens;
    let cache_hit = if cache_denom > 0 { all.cache_read_tokens as f64 / cache_denom as f64 } else { 0.0 };
    let out_in = if all.input_tokens > 0 { all.output_tokens as f64 / all.input_tokens as f64 } else { 0.0 };

    let bar_w = 20usize;
    let cache_label = if cache_hit > 0.6 { "strong" } else if cache_hit > 0.3 { "fair" } else { "low" };
    let cache_c = grade_color(cache_hit, config.cache_alert_ratio, config.cache_alert_ratio * 2.0);
    let (cb_f, cb_e) = smooth_bar(cache_hit, 1.0, bar_w);

    let out_label = if out_in > 0.3 { "strong" } else if out_in > 0.1 { "fair" } else { "low" };
    let out_c = grade_color(out_in, 0.1, 0.3);
    let (ob_f, ob_e) = smooth_bar(out_in, 1.0, bar_w);

    let low_output_sessions = insights.sessions.iter()
        .filter(|s| s.total_input > 0 && (s.total_output as f64 / s.total_input as f64) < 0.05)
        .count();
    let waste_pct = if !insights.sessions.is_empty() {
        low_output_sessions as f64 / insights.sessions.len() as f64 * 100.0
    } else { 0.0 };
    let waste_label = if waste_pct < 15.0 { "ok" } else if waste_pct < 30.0 { "some" } else { "high" };
    let waste_c = grade_color_inverse(waste_pct, 15.0, 30.0);
    let (wb_f, wb_e) = smooth_bar(waste_pct, 100.0, bar_w);

    let econ = vec![
        Line::from(vec![
            Span::styled("   cache hit    ", Style::default().fg(FG_MUTED)),
            Span::styled(cb_f, Style::default().fg(cache_c)),
            Span::styled(cb_e, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:.0}%", cache_hit * 100.0), Style::default().fg(cache_c).bold()),
            Span::styled(format!("  {}", cache_label), Style::default().fg(cache_c)),
        ]),
        Line::from(vec![
            Span::styled("   output/input ", Style::default().fg(FG_MUTED)),
            Span::styled(ob_f, Style::default().fg(out_c)),
            Span::styled(ob_e, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:.0}%", out_in * 100.0), Style::default().fg(out_c).bold()),
            Span::styled(format!("  {}", out_label), Style::default().fg(out_c)),
        ]),
        Line::from(vec![
            Span::styled("   waste sess.  ", Style::default().fg(FG_MUTED)),
            Span::styled(wb_f, Style::default().fg(waste_c)),
            Span::styled(wb_e, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:.0}%", waste_pct), Style::default().fg(waste_c).bold()),
            Span::styled(format!("  {}", waste_label), Style::default().fg(waste_c)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("   avg depth    ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:.1} msgs/session", insights.avg_session_depth), Style::default().fg(FG)),
            Span::styled("      cost/session  ", Style::default().fg(FG_MUTED)),
            Span::styled(
                format!("{} (API eq.)", pricing::format_cost(insights.avg_cost_per_session)),
                Style::default().fg(FG_FAINT),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(econ), chunks[1]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── 24h Activity ──
    let hours = store.by_hour_all();
    let hour_values: Vec<f64> = hours.iter().map(|&h| h as f64).collect();
    let hour_spark = spark(&hour_values);

    let activity_lines = if w >= 80 {
        vec![
            Line::from(vec![
                Span::styled("   activity by hour  ", Style::default().fg(FG_MUTED)),
                Span::styled(hour_spark, Style::default().fg(ACCENT)),
            ]),
            Line::from(vec![
                Span::styled("                     ", Style::default().fg(FG_FAINT)),
                Span::styled(
                    "0     6     12    18    ".to_string(),
                    Style::default().fg(FG_FAINT),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("   activity  ", Style::default().fg(FG_MUTED)),
                Span::styled(hour_spark, Style::default().fg(ACCENT)),
            ]),
        ]
    };
    frame.render_widget(Paragraph::new(activity_lines), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // ── Costliest sessions ──
    let max_rows = chunks[5].height as usize;
    let mut session_lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("   costliest sessions", Style::default().fg(ACCENT)),
            Span::styled(
                format!("{}model    msgs   out/in   cache%    cost",
                    " ".repeat((w as usize).saturating_sub(72).max(2))),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ];

    for s in insights.sessions.iter().take(max_rows.saturating_sub(1)) {
        let ratio = if s.total_input > 0 { s.total_output as f64 / s.total_input as f64 } else { 0.0 };
        let ratio_color = if ratio > 0.3 { Color::Rgb(120, 190, 120) } else if ratio > 0.1 { FG_MUTED } else { YELLOW };

        let cache_total = s.total_cache_read + s.total_input;
        let cache_pct = if cache_total > 0 { s.total_cache_read as f64 / cache_total as f64 * 100.0 } else { 0.0 };
        let cache_color = grade_color(cache_pct / 100.0, config.cache_alert_ratio, config.cache_alert_ratio * 2.0);

        let name_w = (w as usize).saturating_sub(65).max(10);
        session_lines.push(Line::from(vec![
            Span::styled(
                format!("   {:<width$}", truncate(&s.project, name_w), width = name_w),
                Style::default().fg(FG),
            ),
            Span::styled(format!("{:>8}", s.model), Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>7}", s.message_count), Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:>8.2}x", ratio), Style::default().fg(ratio_color)),
            Span::styled(format!("{:>8.0}%", cache_pct), Style::default().fg(cache_color)),
            Span::styled(format!("{:>9}", pricing::format_cost(s.cost)), Style::default().fg(ACCENT)),
        ]));
    }

    frame.render_widget(Paragraph::new(session_lines), chunks[5]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[6]);

    // ── Help ──
    let help = Line::from(vec![
        Span::styled("   esc", Style::default().fg(ACCENT)),
        Span::styled(" back   ", Style::default().fg(FG_MUTED)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[7]);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}
