use crate::config::Config;
use crate::parser::Source;
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;

// ════════════════════════════════════════════════════════════════════════
//  Stats: the retrospective view
//
//  Layout (top to bottom):
//    1. Nav header (2 lines)
//    2. Heatmap hero (11 lines)
//    3. Key numbers: 2 columns (5 lines)
//    4. 30d trend + source split (3 lines)
//    5. Daily cost table (scrollable, fills remaining space)
//    6. Footer: models + badges + efficiency (8 lines)
//    7. Help bar (1 line)
// ════════════════════════════════════════════════════════════════════════

pub fn render(frame: &mut ratatui::Frame, store: &Store, _config: &Config, scroll: usize) {
    let area = frame.area();
    let w = area.width;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),   // nav header
            Constraint::Length(11),  // heatmap
            Constraint::Length(1),   // divider
            Constraint::Length(5),   // key numbers
            Constraint::Length(1),   // divider
            Constraint::Length(3),   // 30d trend + source split
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // daily cost table (scrollable)
            Constraint::Length(1),   // divider
            Constraint::Length(8),   // footer: models + badges + efficiency
            Constraint::Length(1),   // help
        ])
        .split(area);

    // 1. Nav header
    let header = nav_header("stats", w);
    let header_lines: Vec<Line> = header.into_iter().collect();
    frame.render_widget(Paragraph::new(header_lines), chunks[0]);

    // 2. Heatmap
    render_heatmap(frame, store, chunks[1]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // 3. Key numbers
    render_key_numbers(frame, store, chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // 4. 30d trend + source split
    render_trend(frame, store, chunks[5]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[6]);

    // 5. Daily cost table
    render_daily_table(frame, store, chunks[7], scroll);
    frame.render_widget(Paragraph::new(divider(w)), chunks[8]);

    // 6. Footer
    render_footer(frame, store, chunks[9]);

    // 7. Help
    let help = help_bar(&[
        ("esc/b", "back to browser"),
        ("\u{2191}\u{2193}", "scroll daily table"),
        ("d", "browse cc"),
        ("c", "browse cursor"),
        ("?", "help"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[10]);
}

// ── Heatmap: activity contribution grid ──────────────────────────────

fn render_heatmap(frame: &mut ratatui::Frame, store: &Store, area: Rect) {
    let (grid, month_labels) = store.activity_heatmap();
    let total_days = grid.len();
    let weeks = total_days.div_ceil(7);
    let max_count = grid.iter().max().copied().unwrap_or(1).max(1);

    let heatmap_blocks = ['\u{00b7}', '\u{2591}', '\u{2592}', '\u{2593}', '\u{2588}'];
    let day_labels = ["", "Mon", "", "Wed", "", "Fri", ""];

    let mut lines: Vec<Line> = Vec::new();

    // Month labels
    let prefix = "      ";
    let mut month_row = String::from(prefix);
    let mut last_col = 0i32;
    for (label, week_idx) in &month_labels {
        let target = *week_idx;
        if (target as i32) < last_col + 4 && last_col > 0 { continue; }
        while month_row.len() < prefix.len() + target { month_row.push(' '); }
        let short = &label[..3.min(label.len())];
        month_row.push_str(short);
        last_col = target as i32 + short.len() as i32;
    }
    lines.push(Line::from(Span::styled(month_row, Style::default().fg(FG_MUTED))));

    for (row, day_label) in day_labels.iter().enumerate() {
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(format!("{:>4} ", day_label), Style::default().fg(FG_FAINT)));

        for week in 0..weeks {
            let idx = week * 7 + row;
            if idx >= total_days {
                spans.push(Span::styled("\u{00b7}", Style::default().fg(FG_FAINT)));
                continue;
            }
            let count = grid[idx];
            let intensity = if count == 0 { 0 }
                else { ((count as f64 / max_count as f64) * 3.0).ceil() as usize + 1 };
            let ch = heatmap_blocks[intensity.min(4)];
            let color = match intensity {
                0 => FG_FAINT,
                1 => Color::Rgb(80, 120, 80),
                2 => Color::Rgb(100, 155, 100),
                3 => Color::Rgb(120, 180, 120),
                _ => GREEN,
            };
            spans.push(Span::styled(format!("{}", ch), Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }

    // Cost sparkline beneath heatmap
    let daily_costs = store.daily_costs(weeks.min(30));
    let cost_spark = spark(&daily_costs);
    lines.push(Line::from(vec![
        Span::styled("      ", Style::default()),
        Span::styled(cost_spark, Style::default().fg(ACCENT)),
        Span::styled("  cost/day", Style::default().fg(FG_FAINT)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("      Less ", Style::default().fg(FG_FAINT)),
        Span::styled("\u{2591}", Style::default().fg(Color::Rgb(80, 120, 80))),
        Span::styled(" ", Style::default()),
        Span::styled("\u{2592}", Style::default().fg(Color::Rgb(100, 155, 100))),
        Span::styled(" ", Style::default()),
        Span::styled("\u{2593}", Style::default().fg(Color::Rgb(120, 180, 120))),
        Span::styled(" ", Style::default()),
        Span::styled("\u{2588}", Style::default().fg(GREEN)),
        Span::styled(" More", Style::default().fg(FG_FAINT)),
    ]));

    frame.render_widget(Paragraph::new(lines), area);
}

// ── Key numbers: compact 2-column summary ────────────────────────────

fn render_key_numbers(frame: &mut ratatui::Frame, store: &Store, area: Rect) {
    let all = store.all_time();
    let by_src = store.by_source();
    let cc = by_src.get(&Source::ClaudeCode);
    let cu = by_src.get(&Source::Cursor);
    let streak = store.streak_days();
    let longest_streak = store.longest_streak();
    let active_days = store.active_days();
    let total_tokens = store.total_tokens();
    let favorite_model = store.favorite_model().unwrap_or_else(|| "none".to_string());
    let (tw_cost, lw_cost, tw_sess, _) = store.week_comparison();
    let (_, _, projected) = store.month_projection();

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: activity
    let cc_sess = cc.map(|a| a.session_count).unwrap_or(0);
    let cu_sess = cu.map(|a| a.session_count).unwrap_or(0);
    let left = vec![
        Line::from(vec![
            Span::styled("   Sessions  ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("{}", all.session_count), Style::default().fg(FG).bold()),
            Span::styled(format!("  ({} CC  {} Cursor)", cc_sess, cu_sess), Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("   Active days  ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("{}", active_days), Style::default().fg(FG)),
        ]),
        Line::from(vec![
            Span::styled("   Streak  ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("{} days", streak), Style::default().fg(if streak >= 7 { GREEN } else { FG })),
            Span::styled(format!("  (best {})", longest_streak), Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("   Tokens  ", Style::default().fg(FG_FAINT)),
            Span::styled(compact(total_tokens), Style::default().fg(FG)),
        ]),
        Line::from(vec![
            Span::styled("   Model  ", Style::default().fg(FG_FAINT)),
            Span::styled(capitalize(&favorite_model), Style::default().fg(
                match favorite_model.as_str() { "opus" => PURPLE, "sonnet" => ACCENT, "haiku" => ACCENT2, _ => FG }
            )),
        ]),
    ];

    // Right: cost
    let cc_cost = cc.map(|a| a.cost).unwrap_or(0.0);
    let cu_cost = cu.map(|a| a.cost).unwrap_or(0.0);
    let cost_delta = if lw_cost > 0.0 { (tw_cost - lw_cost) / lw_cost * 100.0 } else { 0.0 };
    let delta_color = if cost_delta > 10.0 { RED } else if cost_delta < -10.0 { GREEN } else { FG_MUTED };

    let right = vec![
        Line::from(vec![
            Span::styled("   Total cost  ", Style::default().fg(FG_FAINT)),
            Span::styled(pricing::format_cost(all.cost), Style::default().fg(ACCENT).bold()),
            Span::styled(format!("  ({} CC  {} Cu)", pricing::format_cost(cc_cost), pricing::format_cost(cu_cost)),
                Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("   This week  ", Style::default().fg(FG_FAINT)),
            Span::styled(pricing::format_cost(tw_cost), Style::default().fg(FG)),
            Span::styled(format!("  {} sess  ", tw_sess), Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:+.0}% vs last", cost_delta), Style::default().fg(delta_color)),
        ]),
        Line::from(vec![
            Span::styled("   Mo pace  ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("~{}", pricing::format_cost(projected)), Style::default().fg(YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("   Cache hit  ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("{:.0}%", store.avg_cache_hit_rate() * 100.0), Style::default().fg(
                if store.avg_cache_hit_rate() > 0.7 { GREEN } else if store.avg_cache_hit_rate() > 0.4 { YELLOW } else { RED }
            )),
            Span::styled("   Compactions  ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("{}", store.total_compactions()), Style::default().fg(FG)),
        ]),
        Line::from(vec![
            Span::styled("   Out/$  ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("{:.0} tok", store.output_per_dollar()), Style::default().fg(FG_MUTED)),
            Span::styled("   Bloat  ", Style::default().fg(FG_FAINT)),
            Span::styled(pricing::format_cost(store.total_context_premium()), Style::default().fg(
                if store.total_context_premium() < 50.0 { GREEN } else if store.total_context_premium() < 200.0 { YELLOW } else { RED }
            )),
        ]),
    ];

    frame.render_widget(Paragraph::new(left), cols[0]);
    frame.render_widget(Paragraph::new(right), cols[1]);
}

// ── 30d trend + source split ─────────────────────────────────────────

fn render_trend(frame: &mut ratatui::Frame, store: &Store, area: Rect) {
    let w = area.width;
    let days = store.by_day(30);
    let total_30d: f64 = days.iter().map(|d| d.cost).sum();

    let cumulative: Vec<f64> = {
        let mut costs: Vec<f64> = days.iter().rev().map(|d| d.cost).collect();
        let mut acc = 0.0;
        for c in costs.iter_mut() { acc += *c; *c = acc; }
        costs
    };
    let cum_spark = spark(&cumulative);

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

    let lines = vec![
        Line::from(vec![
            Span::styled("   30d cumulative ", Style::default().fg(FG_FAINT)),
            Span::styled(cum_spark, Style::default().fg(ACCENT)),
            Span::styled(format!("  {}", pricing::format_cost(total_30d)), Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("   source split  ", Style::default().fg(FG_FAINT)),
            Span::styled("\u{2588}".repeat(cc_bar_w), Style::default().fg(ACCENT2)),
            Span::styled("\u{2588}".repeat(cu_bar_w), Style::default().fg(BLUE)),
        ]),
        Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled("\u{25cf}", Style::default().fg(ACCENT2)),
            Span::styled(format!(" CC {}  {} sess", pricing::format_cost(cc_cost), cc_sessions), Style::default().fg(FG_MUTED)),
            Span::styled("   ", Style::default()),
            Span::styled("\u{25cf}", Style::default().fg(BLUE)),
            Span::styled(format!(" Cursor {}  {} sess", pricing::format_cost(cu_cost), cu_sessions), Style::default().fg(FG_MUTED)),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

// ── Daily cost table (scrollable) ────────────────────────────────────

fn render_daily_table(frame: &mut ratatui::Frame, store: &Store, area: Rect, scroll: usize) {
    let days = store.by_day(30);
    let today = chrono::Utc::now().date_naive();
    let max_rows = area.height as usize;
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
        Constraint::Length(11),
        Constraint::Length(9),
        Constraint::Length(bar_w as u16),
        Constraint::Length(5),
        Constraint::Length(8),
        Constraint::Length(8),
    ];
    let day_table = Table::new(day_rows, day_widths)
        .header(day_header)
        .column_spacing(1);
    frame.render_widget(day_table, area);
}

// ── Footer: models + badges + context budget ─────────────────────────

fn render_footer(frame: &mut ratatui::Frame, store: &Store, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),  // models
            Constraint::Percentage(30),  // badges
            Constraint::Percentage(35),  // context budget
        ])
        .split(area);

    // Models
    let models = store.by_model();
    let total_cost: f64 = models.iter().map(|m| m.cost).sum();
    let mut model_lines: Vec<Line> = Vec::new();
    model_lines.push(Line::from(Span::styled("   MODELS", Style::default().fg(FG_FAINT))));

    let bar_w = 8usize;
    for m in models.iter().take(6) {
        let pct = if total_cost > 0.0 { m.cost / total_cost * 100.0 } else { 0.0 };
        let mc = match m.name.as_str() {
            "opus" => PURPLE, "sonnet" => ACCENT, "haiku" => ACCENT2, _ => FG_MUTED,
        };
        let (bf, be) = smooth_bar(pct, 100.0, bar_w);
        model_lines.push(Line::from(vec![
            Span::styled(format!("   {:>6} ", capitalize(&m.name)), Style::default().fg(mc)),
            Span::styled(bf, Style::default().fg(mc)),
            Span::styled(be, Style::default().fg(FG_FAINT)),
            Span::styled(format!(" {:.0}%  {}", pct, pricing::format_cost(m.cost)), Style::default().fg(FG_MUTED)),
        ]));
    }
    frame.render_widget(Paragraph::new(model_lines), cols[0]);

    // Badges
    let all = store.all_time();
    let streak = store.streak_days();
    let longest_streak = store.longest_streak();
    let total_tokens = store.total_tokens();
    let night_ratio = store.night_owl_ratio();

    let achievements = compute_achievements(
        all.session_count, streak, longest_streak,
        total_tokens, night_ratio, all.cost,
    );

    let mut badge_lines: Vec<Line> = Vec::new();
    badge_lines.push(Line::from(Span::styled("   BADGES", Style::default().fg(FG_FAINT))));

    for (icon, name, earned, progress) in &achievements {
        if *earned {
            badge_lines.push(Line::from(vec![
                Span::styled(format!("   {} ", icon), Style::default().fg(ACCENT)),
                Span::styled(name.to_string(), Style::default().fg(ACCENT)),
            ]));
        } else {
            badge_lines.push(Line::from(vec![
                Span::styled(format!("   {} ", icon), Style::default().fg(FG_FAINT)),
                Span::styled(format!("{} ", name), Style::default().fg(FG_FAINT)),
                Span::styled(progress.clone(), Style::default().fg(FG_MUTED)),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(badge_lines), cols[1]);

    // Context budget
    let budget = crate::budget::scan();
    let mut ctx_lines: Vec<Line> = Vec::new();
    ctx_lines.push(Line::from(Span::styled("   CTX BUDGET", Style::default().fg(FG_FAINT))));

    let budget_color = if budget.pct_used < 3.0 { GREEN } else if budget.pct_used < 10.0 { YELLOW } else { RED };
    ctx_lines.push(Line::from(vec![
        Span::styled("   Overhead  ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.1}%", budget.pct_used), Style::default().fg(budget_color)),
        Span::styled(format!(" ({}K tok)", budget.always_tokens / 1000), Style::default().fg(FG_FAINT)),
    ]));
    ctx_lines.push(Line::from(vec![
        Span::styled("   Memories  ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{}", budget.memory_count), Style::default().fg(FG_MUTED)),
        if !budget.stale_items.is_empty() {
            Span::styled(format!(" ({} stale)", budget.stale_items.len()), Style::default().fg(YELLOW))
        } else { Span::styled("", Style::default()) },
    ]));

    let dup = &budget.duplication;
    let dup_color = if dup.duplicate_pct < 5.0 { GREEN } else if dup.duplicate_pct < 15.0 { YELLOW } else { RED };
    ctx_lines.push(Line::from(vec![
        Span::styled("   Dupes  ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.1}%", dup.duplicate_pct), Style::default().fg(dup_color)),
        Span::styled(format!(" (~{}K wasted)", dup.wasted_tokens / 1000), Style::default().fg(FG_FAINT)),
    ]));
    ctx_lines.push(Line::from(vec![
        Span::styled("   Skills  ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{}", budget.skill_count), Style::default().fg(FG_MUTED)),
        Span::styled(format!("   Plugins  {}", budget.plugin_count), Style::default().fg(FG_MUTED)),
    ]));

    frame.render_widget(Paragraph::new(ctx_lines), cols[2]);
}

// ── Helpers ──────────────────────────────────────────────────────────

fn compute_achievements(
    sessions: usize, streak: usize, longest_streak: usize,
    tokens: u64, night_ratio: f64, cost: f64,
) -> Vec<(&'static str, &'static str, bool, String)> {
    vec![
        ("\u{1f525}", "7d Streak", streak >= 7 || longest_streak >= 7,
         format!("{}/7", longest_streak.min(7))),
        ("\u{26a1}", "30d Streak", longest_streak >= 30,
         format!("{}/30", longest_streak.min(30))),
        ("\u{1f4ac}", "100 Sess", sessions >= 100,
         format!("{}/100", sessions.min(100))),
        ("\u{1f30c}", "1M Tokens", tokens >= 1_000_000,
         if tokens >= 1_000_000 { "\u{2713}".into() } else { format!("{:.0}%", tokens as f64 / 1_000_000.0 * 100.0) }),
        ("\u{1f319}", "Night Owl", night_ratio >= 20.0,
         format!("{:.0}%", night_ratio)),
        ("\u{1f4b0}", "$100 Club", cost >= 100.0,
         if cost >= 100.0 { "\u{2713}".into() } else { format!("{:.0}%", cost) }),
        ("\u{1f3af}", "500 Sess", sessions >= 500,
         format!("{}/500", sessions.min(500))),
    ]
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
