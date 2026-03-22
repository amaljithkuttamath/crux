use crate::config::Config;
use crate::parser::Source;
use crate::pricing;
use crate::store::{Store, SessionTimeline};
use super::widgets::*;
use chrono::Timelike;
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct DashboardState {
    pub cursor: usize,
    pub scroll: usize,
    pub detail: Option<SessionDetailView>,
    pub cached_session_ids: Vec<String>,
}

pub struct SessionDetailView {
    pub session_id: String,
    pub timeline: SessionTimeline,
}

impl DashboardState {
    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self, max: usize) {
        if max > 0 && self.cursor + 1 < max {
            self.cursor += 1;
        }
    }

    pub fn enter(&mut self, store: &Store) {
        if self.detail.is_some() { return; }
        if let Some(id) = self.cached_session_ids.get(self.cursor).cloned() {
            if let Some(timeline) = store.session_timeline(&id) {
                self.detail = Some(SessionDetailView {
                    session_id: id,
                    timeline,
                });
            }
        }
    }

    pub fn back(&mut self) -> bool {
        if self.detail.is_some() { self.detail = None; true } else { false }
    }
}


pub fn render(
    frame: &mut ratatui::Frame,
    store: &Store,
    config: &Config,
    state: &mut DashboardState,
    live_sessions: &HashMap<String, bool>,
) {
    if state.detail.is_some() {
        render_detail(frame, store, config, state);
    } else {
        render_main(frame, store, config, state, live_sessions);
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Overview: ticker, source blocks, model usage
// ════════════════════════════════════════════════════════════════════════

fn render_main(
    frame: &mut ratatui::Frame,
    store: &Store,
    config: &Config,
    state: &mut DashboardState,
    live_sessions: &HashMap<String, bool>,
) {
    let area = frame.area();
    let w = area.width;

    let today = store.today();
    let week = store.this_week();

    // --- Gather source data ---
    let cc_sessions: Vec<_> = store.today_sessions_by_source(Source::ClaudeCode)
        .into_iter().filter(|s| !s.is_subagent).collect();
    let cu_sessions: Vec<_> = store.today_sessions_by_source(Source::Cursor)
        .into_iter().filter(|s| !s.is_subagent).collect();

    let has_cc = !cc_sessions.is_empty();
    let has_cu = !cu_sessions.is_empty();

    let cc_agg = store.today_by_source(Source::ClaudeCode);
    let cu_agg = store.today_by_source(Source::Cursor);

    // Source block heights
    let source_lines: u16 = if has_cc || has_cu { 4 } else { 1 };

    // Model bars
    let models = store.today_by_model();
    let model_lines: u16 = if models.is_empty() { 1 } else { 1 + models.len() as u16 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),  // nav header
            Constraint::Length(1),  // ticker line 1
            Constraint::Length(1),  // ticker line 2
            Constraint::Length(1),  // divider
            Constraint::Length(source_lines), // source blocks
            Constraint::Length(1),  // divider
            Constraint::Length(model_lines), // model usage
            Constraint::Length(1),  // divider
            Constraint::Min(3),    // recent sessions
            Constraint::Length(1),  // divider
            Constraint::Length(1),  // help bar
        ])
        .split(area);

    // --- Nav header ---
    let nav = nav_header("overview", w);
    frame.render_widget(Paragraph::new(nav), chunks[0]);

    // --- Ticker line 1 ---
    let spark_data = store.sessions_per_day(7);
    let spark_str = spark(&spark_data);
    let streak = store.streak_days();

    let ticker1 = Line::from(vec![
        Span::styled("   TODAY", Style::default().fg(ACCENT).bold()),
        Span::styled(format!("  {}", pricing::format_cost(today.cost)), Style::default().fg(FG).bold()),
        Span::styled(format!("{}",
            " ".repeat((w as usize).saturating_sub(50).max(1))),
            Style::default()),
        Span::styled(spark_str, Style::default().fg(ACCENT)),
        Span::styled(" 7d", Style::default().fg(FG_FAINT)),
        Span::styled(format!("  streak {}d", streak), Style::default().fg(ACCENT)),
    ]);
    frame.render_widget(Paragraph::new(ticker1), chunks[1]);

    // --- Ticker line 2 ---
    let rolling_avg = store.rolling_avg_daily_cost(7);
    let day_count = store.by_day(7).len();

    let avg_spans: Vec<Span> = if rolling_avg == 0.0 && day_count < 3 {
        vec![
            Span::styled("   vs avg ", Style::default().fg(FG_FAINT)),
            Span::styled("building baseline", Style::default().fg(FG_MUTED)),
        ]
    } else if rolling_avg == 0.0 {
        vec![
            Span::styled("   vs avg ", Style::default().fg(FG_FAINT)),
            Span::styled("no baseline", Style::default().fg(FG_MUTED)),
        ]
    } else {
        let diff_pct = ((today.cost - rolling_avg) / rolling_avg * 100.0) as i64;
        let (label, color) = if diff_pct.unsigned_abs() > 200 {
            if diff_pct > 0 {
                ("well above avg".to_string(), RED)
            } else {
                ("well below avg".to_string(), GREEN)
            }
        } else {
            let c = if diff_pct.unsigned_abs() > 50 { RED }
                else if diff_pct.unsigned_abs() > 20 { YELLOW }
                else { FG_FAINT };
            (format!("{:+}%", diff_pct.clamp(-200, 200)), c)
        };
        vec![
            Span::styled(format!("   vs avg {}", pricing::format_cost(rolling_avg)), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {}", label), Style::default().fg(color)),
        ]
    };

    let hours_elapsed = {
        let now = chrono::Utc::now();
        (now.time().hour() as f64 + now.time().minute() as f64 / 60.0).max(0.1)
    };
    let burn_rate = today.cost / hours_elapsed;

    let mut ticker2_spans = avg_spans;
    ticker2_spans.push(Span::styled(
        format!("    {}/hr", pricing::format_cost(burn_rate)),
        Style::default().fg(FG_MUTED),
    ));

    // Budget gauge
    let (budget_label, budget_pct) = if let Some(budget) = config.budget_daily {
        ("budget", today.cost / budget * 100.0)
    } else if let Some(budget) = config.budget_weekly {
        ("budget", week.cost / budget * 100.0)
    } else {
        ("", 0.0)
    };
    if !budget_label.is_empty() {
        let filled = ((budget_pct / 100.0).clamp(0.0, 1.0) * 6.0).round() as usize;
        let bar_f = "\u{2588}".repeat(filled);
        let bar_e = "\u{2591}".repeat(6_usize.saturating_sub(filled));
        let color = if budget_pct > 90.0 { RED } else if budget_pct > 70.0 { YELLOW } else { GREEN };
        ticker2_spans.push(Span::styled(
            format!("     {} {}{} {:.0}%", budget_label, bar_f, bar_e, budget_pct),
            Style::default().fg(color),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(ticker2_spans)), chunks[2]);

    // --- Divider ---
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // --- Source summary blocks ---
    if !has_cc && !has_cu {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "   no sessions today", Style::default().fg(FG_FAINT),
            ))),
            chunks[4],
        );
    } else {
        // Build source blocks side by side or single
        let both = has_cc && has_cu;
        let source_area = chunks[4];

        if both {
            let halves = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(source_area);
            render_source_block(frame, halves[0], "CC", ACCENT2, &cc_agg, &cc_sessions, store, config, live_sessions, Source::ClaudeCode);
            render_source_block(frame, halves[1], "CURSOR", BLUE, &cu_agg, &cu_sessions, store, config, live_sessions, Source::Cursor);
        } else if has_cc {
            render_source_block(frame, source_area, "CC", ACCENT2, &cc_agg, &cc_sessions, store, config, live_sessions, Source::ClaudeCode);
        } else {
            render_source_block(frame, source_area, "CURSOR", BLUE, &cu_agg, &cu_sessions, store, config, live_sessions, Source::Cursor);
        }
    }

    // --- Divider ---
    frame.render_widget(Paragraph::new(divider(w)), chunks[5]);

    // --- Model usage bars ---
    let model_area = chunks[6];
    if models.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "   no model usage today", Style::default().fg(FG_FAINT),
            ))),
            model_area,
        );
    } else {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            "   MODEL USAGE TODAY", Style::default().fg(ACCENT).bold(),
        )));

        let max_cost = models.first().map(|m| m.cost).unwrap_or(1.0).max(0.01);
        let bar_width = (w as usize).saturating_sub(30).max(5);

        for (i, m) in models.iter().enumerate() {
            let pct = if today.cost > 0.0 { m.cost / today.cost * 100.0 } else { 0.0 };
            let filled = ((m.cost / max_cost) * bar_width as f64).round() as usize;
            let bar = "\u{2588}".repeat(filled);
            let empty = "\u{2591}".repeat(bar_width.saturating_sub(filled));
            let color = model_color(i);

            lines.push(Line::from(vec![
                Span::styled(format!("   {:>8} ", m.name), Style::default().fg(FG_MUTED)),
                Span::styled(bar, Style::default().fg(color)),
                Span::styled(empty, Style::default().fg(FG_FAINT)),
                Span::styled(format!(" {} {:.0}%", pricing::format_cost(m.cost), pct), Style::default().fg(FG_MUTED)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), model_area);
    }

    // --- Divider before recent sessions ---
    frame.render_widget(Paragraph::new(divider(w)), chunks[7]);

    // --- Recent sessions ---
    let recent_area = chunks[8];
    let all_sessions = store.sessions_by_time();
    let today_date = chrono::Utc::now().date_naive();
    let recent: Vec<_> = all_sessions.iter()
        .filter(|s| s.start_time.date_naive() == today_date && !s.is_subagent)
        .collect();

    state.cached_session_ids = recent.iter().map(|s| s.session_id.clone()).collect();
    if !state.cached_session_ids.is_empty() {
        state.cursor = state.cursor.min(state.cached_session_ids.len() - 1);
    }

    let max_rows = recent_area.height as usize;
    let now = chrono::Utc::now();

    // Scroll tracking
    if state.cursor >= state.scroll + max_rows {
        state.scroll = state.cursor.saturating_sub(max_rows - 1);
    }
    if state.cursor < state.scroll { state.scroll = state.cursor; }

    if recent.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "   no sessions today", Style::default().fg(FG_FAINT),
            ))),
            recent_area,
        );
    } else {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            "   RECENT SESSIONS", Style::default().fg(ACCENT).bold(),
        )));

        for (i, session) in recent.iter().skip(state.scroll).take(max_rows.saturating_sub(1)).enumerate() {
            let idx = i + state.scroll;
            let is_selected = idx == state.cursor;
            let is_live = live_sessions.get(&session.session_id).copied().unwrap_or(false);

            let source_color = if session.source == Source::ClaudeCode { ACCENT2 } else { BLUE };
            let model_name = crate::store::simplify_model(&store.session_model(&session.session_id));
            let cost = store.session_cost(&session.session_id);

            let analysis = store.analyze_session(&session.session_id);
            let ceiling = session.context_token_limit;
            let status = analysis.as_ref().map(|a| {
                crate::store::analysis::health_status(a, ceiling, is_live, config.context_warn_pct, config.context_danger_pct)
            }).unwrap_or(crate::store::analysis::HealthStatus::Done);

            let age_str = if is_live {
                let elapsed = (now - session.start_time).num_minutes();
                if elapsed >= 60 { format!("{}h{:02}", elapsed / 60, elapsed % 60) } else { format!("{}m", elapsed.max(1)) }
            } else {
                format_ago(session.end_time)
            };

            let prefix = if is_live { "\u{25b6}" } else if is_selected { "\u{25b8}" } else { " " };
            let name_w = (w as usize).saturating_sub(55).max(8);
            let topic = truncate(&session.first_message, name_w);

            let fg = if is_selected { FG } else { FG_MUTED };
            let cost_fg = if is_selected { ACCENT } else { FG_FAINT };

            lines.push(Line::from(vec![
                Span::styled(format!("   {} ", prefix), Style::default().fg(if is_live { GREEN } else if is_selected { ACCENT } else { FG_FAINT })),
                Span::styled("\u{25cf}", Style::default().fg(source_color)),
                Span::styled(format!(" {:<width$}", topic, width = name_w), Style::default().fg(fg)),
                Span::styled(format!(" {:>6}", model_name), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {:>7}", pricing::format_cost(cost)), Style::default().fg(cost_fg)),
                Span::styled(format!("  {:<7}", status.label()), Style::default().fg(health_color(&status))),
                Span::styled(format!(" {:>6}", age_str), Style::default().fg(FG_FAINT)),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), recent_area);
    }

    // --- Bottom ---
    frame.render_widget(Paragraph::new(divider(w)), chunks[9]);
    frame.render_widget(Paragraph::new(help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("enter", "detail"),
        ("d", "cc"),
        ("c", "cursor"),
        ("h", "history"),
        ("q", "quit"),
    ])), chunks[10]);
}

fn render_source_block(
    frame: &mut ratatui::Frame,
    area: Rect,
    label: &str,
    color: Color,
    agg: &crate::store::Aggregation,
    sessions: &[&crate::parser::conversation::SessionMeta],
    store: &Store,
    config: &Config,
    live_sessions: &HashMap<String, bool>,
    source: Source,
) {
    let mut lines: Vec<Line> = Vec::new();

    // Line 1: label + spend
    let active_count = sessions.iter()
        .filter(|s| live_sessions.get(&s.session_id).copied().unwrap_or(false))
        .count();

    lines.push(Line::from(vec![
        Span::styled(format!("   \u{25cf} {}", label), Style::default().fg(color).bold()),
        Span::styled(format!("  {}", pricing::format_cost(agg.cost)), Style::default().fg(FG).bold()),
    ]));

    // Line 2: session count + active
    let session_count = sessions.len();
    let mut info_spans: Vec<Span> = vec![
        Span::styled(format!("   {} sessions", session_count), Style::default().fg(FG_MUTED)),
    ];
    if active_count > 0 {
        info_spans.push(Span::styled(
            format!("  {} active", active_count),
            Style::default().fg(GREEN),
        ));
    }
    lines.push(Line::from(info_spans));

    // Line 3: savings if > 5%
    let savings = store.today_savings_by_source(source);
    let savings_pct = if agg.cost > 0.0 { savings / agg.cost } else { 0.0 };
    if savings_pct > 0.05 {
        lines.push(Line::from(vec![
            Span::styled(
                format!("   savings {} ({:.0}%)", pricing::format_cost(savings), savings_pct * 100.0),
                Style::default().fg(YELLOW),
            ),
        ]));
    } else {
        lines.push(Line::from(Span::raw("")));
    }

    // Line 4: health alert if any session has context fill > danger threshold
    let has_danger = sessions.iter().any(|s| {
        store.analyze_session(&s.session_id)
            .map(|a| {
                let hs = crate::store::analysis::health_status(
                    &a, s.context_token_limit, true,
                    config.context_warn_pct, config.context_danger_pct,
                );
                hs == crate::store::analysis::HealthStatus::CtxRot
            })
            .unwrap_or(false)
    });
    if has_danger {
        lines.push(Line::from(Span::styled(
            format!("   \u{26a0} session near context limit"),
            Style::default().fg(RED),
        )));
    } else {
        lines.push(Line::from(Span::raw("")));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

// ════════════════════════════════════════════════════════════════════════
//  Session detail view
// ════════════════════════════════════════════════════════════════════════

fn render_detail(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut DashboardState) {
    let area = frame.area();
    let w = area.width;
    let detail = match &state.detail {
        Some(d) => d,
        None => return,
    };

    let sessions = store.sessions_by_time();
    let meta = sessions.iter().find(|s| s.session_id == detail.session_id);
    let analysis = store.analyze_session(&detail.session_id);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // ── Header ──
    if let Some(meta) = meta {
        let cost = detail.timeline.total_cost;
        let dur = detail.timeline.duration_minutes;
        let dur_str = if dur >= 60 { format!("{}h{:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };

        let ceiling = store.session_meta(&detail.session_id).and_then(|m| m.context_token_limit);
        let health = if let Some(ref a) = analysis {
            let status = crate::store::analysis::health_status(a, ceiling, false, config.context_warn_pct, config.context_danger_pct);
            (status.label(), health_color(&status))
        } else {
            ("", FG_FAINT)
        };

        let source_badge = match meta.source {
            Source::ClaudeCode => ("\u{25cf} CC", ACCENT2),
            Source::Cursor => ("\u{25cf} Cu", BLUE),
        };

        let header = vec![
            Line::from(vec![
                Span::styled(format!("   {}", truncate(&meta.first_message, (w as usize).saturating_sub(10))),
                    Style::default().fg(FG).bold()),
            ]),
            Line::from(vec![
                Span::styled(format!("   {}", source_badge.0), Style::default().fg(source_badge.1)),
                Span::styled(format!("  {}", display_project_name(&meta.project)), Style::default().fg(ACCENT)),
                Span::styled(format!("  {}  {}  {}t", dur_str, pricing::format_cost(cost), meta.user_count),
                    Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {}", health.0), Style::default().fg(health.1).bold()),
                if detail.timeline.compaction_count > 0 {
                    Span::styled(format!("  {} compactions", detail.timeline.compaction_count), Style::default().fg(YELLOW))
                } else { Span::raw("") },
            ]),
            Line::from(Span::raw("")),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Context Timeline with cost sparkline ──
    let turns = &detail.timeline.turns;
    let bar_w = (w as usize).saturating_sub(40).max(10);
    let start_time = turns.first().map(|t| t.timestamp).unwrap_or_else(chrono::Utc::now);

    let thresholds = [25.0, 50.0, 75.0, 85.0];
    let mut last_crossed: Option<usize> = None;
    let mut timeline_lines: Vec<Line> = Vec::new();

    for (i, turn) in turns.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == turns.len() - 1;
        let current_threshold = thresholds.iter().rposition(|&t| turn.context_pct >= t);
        let crossed_new = current_threshold != last_crossed;
        if crossed_new { last_crossed = current_threshold; }

        let is_notable = is_first || is_last || turn.is_compaction || turn.cost > 0.50 || crossed_new;
        if !is_notable { continue; }

        let elapsed = (turn.timestamp - start_time).num_minutes();
        let filled = ((turn.context_pct / 100.0) * bar_w as f64).round() as usize;
        let bar_filled: String = "\u{2588}".repeat(filled);
        let bar_empty: String = "\u{2591}".repeat(bar_w.saturating_sub(filled));
        let bar_color = if turn.context_pct > 85.0 { RED } else if turn.context_pct > 60.0 { YELLOW } else { GREEN };

        let event_label = if is_first { "started" }
            else if turn.is_compaction { "\u{2193} compacted" }
            else if is_last { "current" }
            else if turn.context_pct > 85.0 { "\u{26a0} near limit" }
            else if turn.cost > 0.50 { "cost spike" }
            else { "" };
        let event_color = if turn.is_compaction { YELLOW }
            else if turn.context_pct > 85.0 { RED }
            else if turn.cost > 0.50 { YELLOW }
            else if is_last { ACCENT }
            else { FG_FAINT };

        timeline_lines.push(Line::from(vec![
            Span::styled(format!("   {:>3}m ", elapsed), Style::default().fg(FG_FAINT)),
            Span::styled(bar_filled, Style::default().fg(bar_color)),
            Span::styled(bar_empty, Style::default().fg(FG_FAINT)),
            Span::styled(format!(" {:>3.0}%", turn.context_pct), Style::default().fg(FG_MUTED)),
            Span::styled(format!("  {}", compact(turn.context_size)), Style::default().fg(FG_FAINT)),
            if !event_label.is_empty() {
                Span::styled(format!("  {}", event_label), Style::default().fg(event_color))
            } else { Span::raw("") },
        ]));
    }

    // Cost sparkline
    let costs: Vec<f64> = turns.iter().map(|t| t.cost).collect();
    if !costs.is_empty() {
        let (peak_idx, peak_cost) = costs.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, &c)| (i, c)).unwrap_or((0, 0.0));
        timeline_lines.push(Line::from(Span::raw("")));
        timeline_lines.push(Line::from(vec![
            Span::styled("   cost/turn ", Style::default().fg(FG_FAINT)),
            Span::styled(spark(&costs), Style::default().fg(ACCENT)),
            Span::styled(format!("  peak {} at turn {}", pricing::format_cost(peak_cost), peak_idx + 1), Style::default().fg(FG_MUTED)),
        ]));
    }

    // Cost breakdown if we have analysis
    if let Some(ref a) = analysis {
        let cb = &a.cost_breakdown;
        timeline_lines.push(Line::from(vec![
            Span::styled("   cost  ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("out {}  in {}  cache-r {}  cache-w {}",
                pricing::format_cost(cb.output), pricing::format_cost(cb.input),
                pricing::format_cost(cb.cache_read), pricing::format_cost(cb.cache_write)),
                Style::default().fg(FG_MUTED)),
        ]));
    }

    frame.render_widget(Paragraph::new(timeline_lines), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    let help = help_bar(&[("esc", "back"), ("\u{2191}\u{2193}", "scroll"), ("q", "quit")]);
    frame.render_widget(Paragraph::new(help), chunks[4]);
}
