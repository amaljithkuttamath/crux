use crate::config::Config;
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

    // Model count determines how much space models need
    let model_rows = models.len().min(4) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),               // title
            Constraint::Length(1),               // divider
            Constraint::Min(6),                  // daily table (fills remaining)
            Constraint::Length(1),               // divider
            Constraint::Length(1),               // model header
            Constraint::Length(model_rows + 1),  // model breakdown + pricing note
            Constraint::Length(1),               // divider
            Constraint::Length(1),               // help
        ])
        .split(area);

    // ── Title with 14d sparkline ──
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

    // ── Daily table ──
    let today = chrono::Utc::now().date_naive();
    let max_rows = chunks[2].height as usize;
    let clamped_scroll = scroll.min(days.len().saturating_sub(max_rows));

    // Find max cost for the mini bar
    let max_cost = days.iter().map(|d| d.cost).fold(0.0f64, f64::max).max(0.01);
    let bar_w = 8usize;

    let mut lines: Vec<Line> = Vec::new();

    // Header row
    lines.push(Line::from(vec![
        Span::styled(format!("   {:<11}", "date"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:>8}", "cost"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>bar_w$}", ""), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>6}", "sess"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>9}", "input"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>9}", "output"), Style::default().fg(FG_FAINT)),
        Span::styled(format!("  {:>9}", "cache"), Style::default().fg(FG_FAINT)),
    ]));

    for day in days.iter().skip(clamped_scroll).take(max_rows.saturating_sub(1)) {
        let is_today = day.date == today;
        let is_yesterday = day.date == today - chrono::Duration::days(1);
        let fg = if is_today { FG } else { FG_MUTED };
        let cost_fg = if is_today { ACCENT } else { FG_MUTED };
        let cache = day.cache_creation_tokens + day.cache_read_tokens;

        let date_label = if is_today {
            "today".to_string()
        } else if is_yesterday {
            "yesterday".to_string()
        } else {
            day.date.format("%b %d %a").to_string()
        };

        let (bf, be) = smooth_bar(day.cost, max_cost, bar_w);
        let bar_color = if is_today { ACCENT } else { FG_FAINT };

        lines.push(Line::from(vec![
            Span::styled(format!("   {:<11}", date_label), Style::default().fg(fg)),
            Span::styled(format!("{:>8}", pricing::format_cost(day.cost)), Style::default().fg(cost_fg)),
            Span::styled(format!("  {}", bf), Style::default().fg(bar_color)),
            Span::styled(be, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>6}", day.session_count), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>9}", compact(day.input_tokens)), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>9}", compact(day.output_tokens)), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>9}", compact(cache)), Style::default().fg(FG_FAINT)),
        ]));
    }

    // Scroll indicator
    if days.len() > max_rows.saturating_sub(1) {
        let remaining = days.len().saturating_sub(clamped_scroll + max_rows - 1);
        if remaining > 0 {
            if let Some(last) = lines.last_mut() {
                *last = Line::from(vec![
                    Span::styled(
                        format!("   ... {} more days", remaining),
                        Style::default().fg(FG_FAINT),
                    ),
                ]);
            }
        }
    }

    frame.render_widget(Paragraph::new(lines), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

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
    frame.render_widget(Paragraph::new(model_header), chunks[4]);

    let mut model_lines: Vec<Line> = Vec::new();
    for m in models.iter().take(model_rows as usize) {
        let cost_pct = if total_cost > 0.0 { m.cost / total_cost * 100.0 } else { 0.0 };
        let total_tok = m.input_tokens + m.output_tokens;
        let alloc_filled = (cost_pct / 100.0 * 5.0).round() as usize;
        let alloc_bar: String = "\u{2588}".repeat(alloc_filled.min(5));
        let alloc_empty: String = "\u{2591}".repeat(5usize.saturating_sub(alloc_filled));

        model_lines.push(Line::from(vec![
            Span::styled(format!("   {:<12}", m.name), Style::default().fg(FG)),
            Span::styled(
                format!("{}{:>8}", " ".repeat((w as usize).saturating_sub(60).max(2)), m.record_count),
                Style::default().fg(FG_FAINT),
            ),
            Span::styled(format!("  {:>10}", compact(total_tok)), Style::default().fg(FG_MUTED)),
            Span::styled(format!("    {:>8}", pricing::format_cost(m.cost)), Style::default().fg(ACCENT)),
            Span::styled(format!("  {}", alloc_bar), Style::default().fg(ACCENT)),
            Span::styled(alloc_empty, Style::default().fg(FG_FAINT)),
            Span::styled(format!("{:>4.0}%", cost_pct), Style::default().fg(FG_FAINT)),
        ]));
    }

    // Pricing reference note
    model_lines.push(Line::from(vec![
        Span::styled("   api-equivalent pricing: ", Style::default().fg(FG_FAINT)),
        Span::styled("sonnet $3/$15", Style::default().fg(FG_FAINT)),
        Span::styled("  opus $15/$75", Style::default().fg(FG_FAINT)),
        Span::styled("  haiku $0.80/$4", Style::default().fg(FG_FAINT)),
        Span::styled("  (per 1M in/out)", Style::default().fg(FG_FAINT)),
    ]));

    frame.render_widget(Paragraph::new(model_lines), chunks[5]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[6]);

    // ── Help ──
    let help = help_bar(&[
        ("\u{2191}\u{2193}", "scroll"),
        ("esc", "back"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[7]);
}
