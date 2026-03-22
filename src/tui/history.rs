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

    // ── Nav header ──
    let total_30d: f64 = days.iter().map(|d| d.cost).sum();
    let header = nav_header("history", w);
    frame.render_widget(Paragraph::new(header[0].clone()), chunks[0]);
    frame.render_widget(Paragraph::new(header[1].clone()), chunks[1]);

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

    // ── Daily cost table ──
    let today = chrono::Utc::now().date_naive();
    let max_rows = chunks[4].height as usize;
    let clamped_scroll = scroll.min(days.len().saturating_sub(max_rows));
    let max_cost = days.iter().map(|d| d.cost).fold(0.0f64, f64::max).max(0.01);
    let bar_w = 12usize;

    let day_header = Row::new(["DATE", "COST", "", "SESS", "INPUT", "OUTPUT"]
        .map(|h| Cell::from(Span::styled(h, Style::default().fg(FG_FAINT)))));

    let mut day_rows: Vec<Row> = Vec::new();
    for day in days.iter().skip(clamped_scroll).take(max_rows.saturating_sub(1)) {
        let is_today = day.date == today;
        let is_yesterday = day.date == today - chrono::Duration::days(1);
        let fg = if is_today { FG } else { FG_MUTED };
        let cost_fg = if is_today { ACCENT } else { FG_MUTED };
        let bar_color = if is_today { ACCENT } else { FG_MUTED };

        let date_label = if is_today { "today".into() }
            else if is_yesterday { "yesterday".into() }
            else { day.date.format("%b %d %a").to_string() };

        let (bf, be) = smooth_bar(day.cost, max_cost, bar_w);

        day_rows.push(Row::new(vec![
            Cell::from(Span::styled(date_label, Style::default().fg(fg))),
            Cell::from(Span::styled(pricing::format_cost(day.cost), Style::default().fg(cost_fg))),
            Cell::from(Line::from(vec![
                Span::styled(bf, Style::default().fg(bar_color)),
                Span::styled(be, Style::default().fg(FG_FAINT)),
            ])),
            Cell::from(Span::styled(day.session_count.to_string(), Style::default().fg(FG_FAINT))),
            Cell::from(Span::styled(compact(day.input_tokens), Style::default().fg(FG_FAINT))),
            Cell::from(Span::styled(compact(day.output_tokens), Style::default().fg(FG_FAINT))),
        ]));
    }

    if days.len() > max_rows.saturating_sub(1) {
        let remaining = days.len().saturating_sub(clamped_scroll + max_rows - 1);
        if remaining > 0 {
            day_rows.push(Row::new(vec![
                Cell::from(Span::styled(format!("... {} more days", remaining), Style::default().fg(FG_FAINT))),
            ]));
        }
    }

    let day_widths = [
        Constraint::Length(11),           // DATE
        Constraint::Length(9),            // COST
        Constraint::Length(bar_w as u16), // bar
        Constraint::Length(5),            // SESS
        Constraint::Length(8),            // INPUT
        Constraint::Length(8),            // OUTPUT
    ];
    let day_table = Table::new(day_rows, day_widths)
        .header(day_header)
        .column_spacing(1);
    frame.render_widget(day_table, chunks[4]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[5]);

    // ── Model breakdown table ──
    let total_cost: f64 = models.iter().map(|m| m.cost).sum();

    let model_header = Row::new(["MODEL", "REQUESTS", "TOKENS", "COST", "SHARE", ""]
        .map(|h| Cell::from(Span::styled(h, Style::default().fg(FG_FAINT)))));

    let mut model_table_rows: Vec<Row> = Vec::new();
    for m in models.iter().take(model_rows as usize) {
        let cost_pct = if total_cost > 0.0 { m.cost / total_cost * 100.0 } else { 0.0 };
        let total_tok = m.input_tokens + m.output_tokens;
        let mc = match m.name.as_str() {
            "opus" => PURPLE, "sonnet" => ACCENT, "haiku" => ACCENT2, _ => FG,
        };

        model_table_rows.push(Row::new(vec![
            Cell::from(Span::styled(m.name.clone(), Style::default().fg(mc))),
            Cell::from(Span::styled(m.record_count.to_string(), Style::default().fg(FG_FAINT))),
            Cell::from(Span::styled(compact(total_tok), Style::default().fg(FG_MUTED))),
            Cell::from(Span::styled(pricing::format_cost(m.cost), Style::default().fg(ACCENT))),
            Cell::from(Span::styled(format!("{:.0}%", cost_pct), Style::default().fg(FG_FAINT))),
            Cell::from(Line::from(mini_bar(cost_pct))),
        ]));
    }

    // API equivalent row
    model_table_rows.push(Row::new(vec![
        Cell::from(Span::styled("api-equivalent:", Style::default().fg(FG_FAINT))),
        Cell::from(Span::styled("sonnet $3/$15", Style::default().fg(FG_FAINT))),
        Cell::from(Span::styled("opus $15/$75", Style::default().fg(FG_FAINT))),
        Cell::from(Span::styled("haiku $0.80/$4", Style::default().fg(FG_FAINT))),
        Cell::from(Span::styled("(per 1M in/out)", Style::default().fg(FG_FAINT))),
        Cell::from(Span::raw("")),
    ]));

    let model_widths = [
        Constraint::Length(12),  // MODEL
        Constraint::Length(10),  // REQUESTS
        Constraint::Length(10),  // TOKENS
        Constraint::Length(10),  // COST
        Constraint::Length(5),   // SHARE %
        Constraint::Length(5),   // mini-bar
    ];

    // Merge header + model rows area
    let model_rect = Rect {
        x: chunks[6].x, y: chunks[6].y,
        width: chunks[6].width,
        height: chunks[6].height + chunks[7].height,
    };
    let model_table = Table::new(model_table_rows, model_widths)
        .header(model_header)
        .column_spacing(1);
    frame.render_widget(model_table, model_rect);
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
