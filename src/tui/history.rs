use crate::config::Config;
use crate::parser::Source;
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut ratatui::Frame, store: &Store, _config: &Config, scroll: usize) {
    let area = frame.area();
    let w = area.width;

    let days = store.by_day(30);
    let models = store.by_model();

    let model_rows = models.len().min(4) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),               // title
            Constraint::Length(1),               // divider
            Constraint::Length(5),               // cumulative trend + source split
            Constraint::Length(1),               // divider
            Constraint::Min(6),                  // daily bars
            Constraint::Length(1),               // divider
            Constraint::Length(1),               // model header
            Constraint::Length(model_rows + 1),  // models
            Constraint::Length(1),               // divider
            Constraint::Length(1),               // help
        ])
        .split(area);

    // ── Title with sparkline ──
    let daily_costs: Vec<f64> = days.iter().take(14).rev().map(|d| d.cost).collect();
    let spark_str = spark(&daily_costs);
    let total_30d: f64 = days.iter().map(|d| d.cost).sum();

    let title = Line::from(vec![
        Span::styled("   history", Style::default().fg(ACCENT).bold()),
        Span::styled(
            format!("{}14d ", " ".repeat((w as usize).saturating_sub(50))),
            Style::default().fg(FG_FAINT),
        ),
        Span::styled(spark_str, Style::default().fg(ACCENT)),
        Span::styled(
            format!("  {} 30d total", pricing::format_cost(total_30d)),
            Style::default().fg(FG_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Cumulative trend + source split ──
    let mut trend_lines: Vec<Line> = Vec::new();

    // 30-day cumulative cost line (text sparkline)
    let cumulative: Vec<f64> = {
        let mut costs: Vec<f64> = days.iter().rev().map(|d| d.cost).collect();
        let mut acc = 0.0;
        for c in costs.iter_mut() {
            acc += *c;
            *c = acc;
        }
        costs
    };
    let cum_spark = spark(&cumulative);
    trend_lines.push(Line::from(vec![
        Span::styled("   30d cumulative ", Style::default().fg(FG_FAINT)),
        Span::styled(cum_spark, Style::default().fg(ACCENT)),
        Span::styled(format!("  {}", pricing::format_cost(total_30d)), Style::default().fg(FG_MUTED)),
    ]));

    // Source split: CC vs Cursor per day (last 7 days)
    let sources = store.by_source();
    let cc_cost = sources.get(&Source::ClaudeCode).map(|a| a.cost).unwrap_or(0.0);
    let cu_cost = sources.get(&Source::Cursor).map(|a| a.cost).unwrap_or(0.0);
    let cc_sessions = sources.get(&Source::ClaudeCode).map(|a| a.session_count).unwrap_or(0);
    let cu_sessions = sources.get(&Source::Cursor).map(|a| a.session_count).unwrap_or(0);

    let total = (cc_cost + cu_cost).max(0.01);
    let cc_pct = cc_cost / total * 100.0;
    let bar_total = (w as usize).saturating_sub(30).max(10);
    let cc_bar_w = ((cc_pct / 100.0) * bar_total as f64).round() as usize;
    let cu_bar_w = bar_total.saturating_sub(cc_bar_w);

    trend_lines.push(Line::from(Span::raw("")));
    trend_lines.push(Line::from(vec![
        Span::styled("   source split  ", Style::default().fg(FG_FAINT)),
        Span::styled("\u{2588}".repeat(cc_bar_w), Style::default().fg(ACCENT2)),
        Span::styled("\u{2588}".repeat(cu_bar_w), Style::default().fg(BLUE)),
    ]));
    trend_lines.push(Line::from(vec![
        Span::styled("   ", Style::default()),
        Span::styled("\u{25cf}", Style::default().fg(ACCENT2)),
        Span::styled(format!(" CC {}  {} sess", pricing::format_cost(cc_cost), cc_sessions), Style::default().fg(FG_MUTED)),
        Span::styled("   ", Style::default()),
        Span::styled("\u{25cf}", Style::default().fg(BLUE)),
        Span::styled(format!(" Cursor {}  {} sess", pricing::format_cost(cu_cost), cu_sessions), Style::default().fg(FG_MUTED)),
    ]));

    while trend_lines.len() < 5 { trend_lines.push(Line::from(Span::raw(""))); }
    frame.render_widget(Paragraph::new(trend_lines), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // ── Daily cost bars ──
    let today = chrono::Utc::now().date_naive();
    let max_rows = chunks[4].height as usize;
    let clamped_scroll = scroll.min(days.len().saturating_sub(max_rows));
    let max_cost = days.iter().map(|d| d.cost).fold(0.0f64, f64::max).max(0.01);
    let bar_w = 12usize;

    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(vec![
        Span::styled(format!("   {:<11}", "date"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:>8}", "cost"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>bar_w$}", ""), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>6}", "sess"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>9}", "input"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>9}", "output"), Style::default().fg(FG_FAINT)),
    ]));

    for day in days.iter().skip(clamped_scroll).take(max_rows.saturating_sub(1)) {
        let is_today = day.date == today;
        let is_yesterday = day.date == today - chrono::Duration::days(1);
        let fg = if is_today { FG } else { FG_MUTED };
        let cost_fg = if is_today { ACCENT } else { FG_MUTED };

        let date_label = if is_today { "today".to_string() }
            else if is_yesterday { "yesterday".to_string() }
            else { day.date.format("%b %d %a").to_string() };

        let (bf, be) = smooth_bar(day.cost, max_cost, bar_w);
        let bar_color = if is_today { ACCENT } else { FG_MUTED };

        lines.push(Line::from(vec![
            Span::styled(format!("   {:<11}", date_label), Style::default().fg(fg)),
            Span::styled(format!("{:>8}", pricing::format_cost(day.cost)), Style::default().fg(cost_fg)),
            Span::styled(format!("  {}", bf), Style::default().fg(bar_color)),
            Span::styled(be, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>6}", day.session_count), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>9}", compact(day.input_tokens)), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>9}", compact(day.output_tokens)), Style::default().fg(FG_FAINT)),
        ]));
    }

    if days.len() > max_rows.saturating_sub(1) {
        let remaining = days.len().saturating_sub(clamped_scroll + max_rows - 1);
        if remaining > 0 {
            if let Some(last) = lines.last_mut() {
                *last = Line::from(vec![
                    Span::styled(format!("   ... {} more days", remaining), Style::default().fg(FG_FAINT)),
                ]);
            }
        }
    }

    frame.render_widget(Paragraph::new(lines), chunks[4]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[5]);

    // ── Model breakdown ──
    let total_cost: f64 = models.iter().map(|m| m.cost).sum();

    let model_header = Line::from(vec![
        Span::styled("   models", Style::default().fg(ACCENT)),
        Span::styled(
            format!("{}requests     tokens        cost    share",
                " ".repeat((w as usize).saturating_sub(60).max(2))),
            Style::default().fg(FG_FAINT),
        ),
    ]);
    frame.render_widget(Paragraph::new(model_header), chunks[6]);

    let mut model_lines: Vec<Line> = Vec::new();
    for m in models.iter().take(model_rows as usize) {
        let cost_pct = if total_cost > 0.0 { m.cost / total_cost * 100.0 } else { 0.0 };
        let total_tok = m.input_tokens + m.output_tokens;
        let alloc_filled = (cost_pct / 100.0 * 5.0).round() as usize;
        let alloc_bar: String = "\u{2588}".repeat(alloc_filled.min(5));
        let alloc_empty: String = "\u{2591}".repeat(5usize.saturating_sub(alloc_filled));

        let model_color = match m.name.as_str() {
            "opus" => PURPLE,
            "sonnet" => ACCENT,
            "haiku" => ACCENT2,
            _ => FG,
        };

        model_lines.push(Line::from(vec![
            Span::styled(format!("   {:<12}", m.name), Style::default().fg(model_color)),
            Span::styled(
                format!("{}{:>8}", " ".repeat((w as usize).saturating_sub(60).max(2)), m.record_count),
                Style::default().fg(FG_FAINT),
            ),
            Span::styled(format!("  {:>10}", compact(total_tok)), Style::default().fg(FG_MUTED)),
            Span::styled(format!("    {:>8}", pricing::format_cost(m.cost)), Style::default().fg(ACCENT)),
            Span::styled(format!("  {}", alloc_bar), Style::default().fg(model_color)),
            Span::styled(alloc_empty, Style::default().fg(FG_FAINT)),
            Span::styled(format!("{:>4.0}%", cost_pct), Style::default().fg(FG_FAINT)),
        ]));
    }

    model_lines.push(Line::from(vec![
        Span::styled("   api-equivalent: ", Style::default().fg(FG_FAINT)),
        Span::styled("sonnet $3/$15", Style::default().fg(FG_FAINT)),
        Span::styled("  opus $15/$75", Style::default().fg(FG_FAINT)),
        Span::styled("  haiku $0.80/$4", Style::default().fg(FG_FAINT)),
        Span::styled("  (per 1M in/out)", Style::default().fg(FG_FAINT)),
    ]));

    frame.render_widget(Paragraph::new(model_lines), chunks[7]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[8]);

    let help = help_bar(&[
        ("\u{2191}\u{2193}", "scroll"),
        ("esc", "back"),
        ("d", "claude code"),
        ("c", "cursor"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[9]);
}
