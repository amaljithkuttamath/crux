use crate::config::Config;
use crate::parser::Source;
use crate::pricing;
use crate::store::{Store, SessionTimeline};
use crate::store::analysis;
use super::widgets::*;
use chrono::Timelike;
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct DashboardState {
    pub cursor: usize,
    #[allow(dead_code)]
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
        render_detail(frame, store, config, state, live_sessions);
    } else {
        render_main(frame, store, config, state, live_sessions);
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Overview: daily cockpit (v4 redesign)
//  Sections: ticker, source split, active sessions, hourly heatmap, projects
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

    // Gather active sessions (live, non-subagent)
    let all_sessions = store.sessions_by_time();
    let active: Vec<_> = all_sessions.iter()
        .filter(|s| !s.is_subagent && live_sessions.get(&s.session_id).copied().unwrap_or(false))
        .collect();

    let active_lines: u16 = if active.is_empty() { 1 } else { 1 + active.len() as u16 };

    // Source counts
    let cc_sessions: Vec<_> = store.today_sessions_by_source(Source::ClaudeCode)
        .into_iter().filter(|s| !s.is_subagent).collect();
    let cu_sessions: Vec<_> = store.today_sessions_by_source(Source::Cursor)
        .into_iter().filter(|s| !s.is_subagent).collect();
    let cc_agg = store.today_by_source(Source::ClaudeCode);
    let cu_agg = store.today_by_source(Source::Cursor);
    let cc_active = cc_sessions.iter().filter(|s| live_sessions.get(&s.session_id).copied().unwrap_or(false)).count();
    let cu_active = cu_sessions.iter().filter(|s| live_sessions.get(&s.session_id).copied().unwrap_or(false)).count();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),           // nav header
            Constraint::Length(1),           // blank
            Constraint::Length(1),           // ticker line 1
            Constraint::Length(1),           // ticker line 2 (budget)
            Constraint::Length(1),           // blank
            Constraint::Length(1),           // source split
            Constraint::Length(1),           // blank
            Constraint::Length(active_lines), // active sessions
            Constraint::Length(1),           // blank
            Constraint::Length(2),           // hourly heatmap
            Constraint::Length(1),           // blank
            Constraint::Min(3),             // project table
            Constraint::Length(1),           // help bar
        ])
        .split(area);

    // ── Nav header ──
    let nav = nav_header("overview", w);
    frame.render_widget(Paragraph::new(nav), chunks[0]);

    // ── Ticker line 1: TODAY $X vs avg $Y +Z% $X/hr spark streak ──
    let spark_data = store.daily_costs(7);
    let spark_str = spark(&spark_data);
    let streak = store.streak_days();
    let rolling_avg = store.rolling_avg_daily_cost(7);

    let hours_elapsed = {
        let now = chrono::Utc::now();
        (now.time().hour() as f64 + now.time().minute() as f64 / 60.0).max(0.1)
    };
    let burn_rate = today.cost / hours_elapsed;

    let mut ticker1_spans: Vec<Span> = vec![
        Span::styled("   TODAY", Style::default().fg(ACCENT).bold()),
        Span::styled(format!("  {}", pricing::format_cost(today.cost)), Style::default().fg(ACCENT).bold()),
    ];

    // vs avg with delta
    if rolling_avg > 0.0 {
        let diff_pct = ((today.cost - rolling_avg) / rolling_avg * 100.0) as i64;
        let delta_color = if diff_pct > 50 { RED } else if diff_pct < -20 { GREEN } else { FG_FAINT };
        ticker1_spans.push(Span::styled(
            format!("     vs avg {}", pricing::format_cost(rolling_avg)),
            Style::default().fg(FG_FAINT),
        ));
        ticker1_spans.push(Span::styled(
            format!("  {:+}%", diff_pct.clamp(-200, 200)),
            Style::default().fg(delta_color),
        ));
    }

    ticker1_spans.push(Span::styled(
        format!("     {}/hr", pricing::format_cost(burn_rate)),
        Style::default().fg(FG_MUTED),
    ));

    // Right-align spark + streak
    let left_len: usize = ticker1_spans.iter().map(|s| s.content.len()).sum();
    let right_len = spark_str.chars().count() + 3 + format!("  streak {}d", streak).len();
    let pad = (w as usize).saturating_sub(left_len + right_len + 2).max(1);
    ticker1_spans.push(Span::styled(" ".repeat(pad), Style::default()));
    ticker1_spans.push(Span::styled(spark_str, Style::default().fg(ACCENT)));
    ticker1_spans.push(Span::styled(" 7d", Style::default().fg(FG_FAINT)));
    ticker1_spans.push(Span::styled(format!("  streak {}d", streak), Style::default().fg(ACCENT)));

    frame.render_widget(Paragraph::new(Line::from(ticker1_spans)), chunks[2]);

    // ── Ticker line 2: budget bullet bar ──
    let (budget_label, budget_pct) = if let Some(budget) = config.budget_daily {
        ("budget", today.cost / budget * 100.0)
    } else if let Some(budget) = config.budget_weekly {
        ("budget", week.cost / budget * 100.0)
    } else {
        ("", 0.0)
    };

    if !budget_label.is_empty() {
        let bar_w = (w as usize).saturating_sub(20).clamp(10, 30);
        let (bf, be) = smooth_bar(budget_pct, 100.0, bar_w);
        let color = if budget_pct > 90.0 { RED } else if budget_pct > 70.0 { YELLOW } else { GREEN };
        frame.render_widget(Paragraph::new(Line::from(vec![
            Span::styled("       ", Style::default()),
            Span::styled(bf, Style::default().fg(color)),
            Span::styled(be, Style::default().fg(FG_FAINT)),
            Span::styled(format!(" {} {:.0}%", budget_label, budget_pct), Style::default().fg(color)),
        ])), chunks[3]);
    }

    // ── Source split (1 line) ──
    let mut src_spans: Vec<Span> = vec![Span::styled("   ", Style::default())];
    let has_cc = !cc_sessions.is_empty() || cc_agg.cost > 0.0;
    let has_cu = !cu_sessions.is_empty() || cu_agg.cost > 0.0;

    if has_cc {
        src_spans.push(Span::styled("\u{25cf}", Style::default().fg(ACCENT2)));
        src_spans.push(Span::styled(
            format!(" CC  {}  {} sess", pricing::format_cost(cc_agg.cost), cc_sessions.len()),
            Style::default().fg(FG_MUTED),
        ));
        if cc_active > 0 {
            src_spans.push(Span::styled(format!("  {} active", cc_active), Style::default().fg(GREEN)));
        }
        // Today's total compactions across CC sessions
        let today_compactions: usize = cc_sessions.iter()
            .filter_map(|s| store.analyze_session(&s.session_id))
            .map(|a| a.compaction_count)
            .sum();
        if today_compactions > 0 {
            src_spans.push(Span::styled(
                format!("  {}c", today_compactions),
                Style::default().fg(YELLOW),
            ));
        }
    }
    if has_cu {
        if has_cc { src_spans.push(Span::styled("          ", Style::default())); }
        src_spans.push(Span::styled("\u{25cf}", Style::default().fg(BLUE)));
        src_spans.push(Span::styled(
            format!(" Cu  {}  {} sess", pricing::format_cost(cu_agg.cost), cu_sessions.len()),
            Style::default().fg(FG_MUTED),
        ));
        if cu_active > 0 {
            src_spans.push(Span::styled(format!("  {} active", cu_active), Style::default().fg(GREEN)));
        }
    }
    if !has_cc && !has_cu {
        src_spans.push(Span::styled("no sessions today", Style::default().fg(FG_FAINT)));
    }
    frame.render_widget(Paragraph::new(Line::from(src_spans)), chunks[5]);

    // ── Active sessions ──
    let now = chrono::Utc::now();
    state.cached_session_ids = active.iter().map(|s| s.session_id.clone()).collect();
    if !state.cached_session_ids.is_empty() {
        state.cursor = state.cursor.min(state.cached_session_ids.len() - 1);
    }

    if active.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "   ACTIVE SESSIONS  none", Style::default().fg(FG_FAINT),
            ))),
            chunks[7],
        );
    } else {
        let header_row = Row::new(["SESSION", "MODEL", "COST", "CTX", "STATUS", "AGE"]
            .map(|h| Cell::from(Span::styled(h, Style::default().fg(FG_FAINT)))));

        let table_rows: Vec<Row> = active.iter().enumerate().map(|(i, session)| {
            let is_selected = i == state.cursor;
            let model_name = crate::store::simplify_model(&store.session_model(&session.session_id));
            let cost = store.session_cost(&session.session_id);
            let ana = store.analyze_session(&session.session_id);
            let ceiling = session.context_token_limit;

            let ctx_pct = if let Some(ref a) = ana {
                if let Some(ceil) = ceiling {
                    (a.context_current as f64 / ceil as f64 * 100.0).min(100.0)
                } else if a.context_peak > 0 {
                    (a.context_current as f64 / a.context_peak as f64 * 100.0).min(100.0)
                } else { 0.0 }
            } else { 0.0 };

            let status = ana.as_ref().map(|a| {
                analysis::health_status(a, ceiling, true, config.context_warn_pct, config.context_danger_pct)
            }).unwrap_or(analysis::HealthStatus::Fresh);

            let compactions = ana.as_ref().map(|a| a.compaction_count).unwrap_or(0);
            let elapsed = (now - session.start_time).num_minutes();
            let age_str = if elapsed >= 60 { format!("{}h{:02}", elapsed / 60, elapsed % 60) } else { format!("{}m", elapsed.max(1)) };

            let fg = if is_selected { FG } else { FG_MUTED };
            let prefix = if is_selected { "\u{25b8} " } else { "  " };

            let mut status_spans = vec![
                Span::styled(status.label().to_string(), Style::default().fg(health_color(&status))),
            ];
            if compactions > 0 {
                status_spans.push(Span::styled(format!(" {}c", compactions), Style::default().fg(YELLOW)));
            }

            Row::new(vec![
                Cell::from(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
                    Span::styled(session.first_message.clone(), Style::default().fg(fg)),
                ])),
                Cell::from(Span::styled(model_name, Style::default().fg(FG_FAINT))),
                Cell::from(Span::styled(pricing::format_cost(cost), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT }))),
                Cell::from(Line::from(mini_bar(ctx_pct))),
                Cell::from(Line::from(status_spans)),
                Cell::from(Span::styled(age_str, Style::default().fg(FG_FAINT))),
            ])
        }).collect();

        let widths = [
            Constraint::Min(15),     // SESSION
            Constraint::Length(7),   // MODEL
            Constraint::Length(8),   // COST
            Constraint::Length(5),   // CTX
            Constraint::Length(10),  // STATUS (+compaction)
            Constraint::Length(6),   // AGE
        ];

        let table = Table::new(table_rows, widths)
            .header(header_row)
            .column_spacing(1);
        frame.render_widget(table, chunks[7]);
    }

    // ── Hourly activity heatmap (2 lines) ──
    let hourly = store.today_by_hour();
    let peak_cost = hourly.iter().map(|(c, _)| *c).fold(0.0f64, f64::max).max(0.01);

    let mut heatmap_chars = String::new();
    for &(cost, _) in hourly.iter().take(24).skip(6) {
        let ratio = cost / peak_cost;
        let ch = if ratio <= 0.0 { '\u{00b7}' }
            else if ratio < 0.5 { '\u{25aa}' }
            else { '\u{2588}' };
        heatmap_chars.push(ch);
    }

    let mut hour_labels = String::new();
    for h in 6..=23 {
        if h % 3 == 0 {
            hour_labels.push_str(&format!("{:<3}", h));
        } else {
            hour_labels.push_str("   ");
        }
    }

    let heatmap_line1 = Line::from(vec![
        Span::styled("     ", Style::default()),
        Span::styled(heatmap_chars, Style::default().fg(ACCENT)),
        Span::styled("   activity today", Style::default().fg(FG_FAINT)),
    ]);
    let heatmap_line2 = Line::from(vec![
        Span::styled("     ", Style::default()),
        Span::styled(hour_labels.trim_end().to_string(), Style::default().fg(FG_FAINT)),
    ]);
    frame.render_widget(Paragraph::new(vec![heatmap_line1, heatmap_line2]), chunks[9]);

    // ── Project table (remaining space) ──
    let projects = store.by_project_cost();
    let project_area = chunks[11];
    let max_rows = project_area.height as usize;

    if projects.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "   no projects", Style::default().fg(FG_FAINT),
            ))),
            project_area,
        );
    } else {
        let max_project_cost = projects.first().map(|p| p.cost).unwrap_or(1.0).max(0.01);
        let total_cost: f64 = projects.iter().map(|p| p.cost).sum();
        let bar_w = 10usize;

        let proj_header = Row::new(["PROJECT", "", "COST", "%", "SESS", "LAST"]
            .map(|h| Cell::from(Span::styled(h, Style::default().fg(FG_FAINT)))));

        let proj_rows: Vec<Row> = projects.iter().take(max_rows.saturating_sub(1)).map(|p| {
            let pname = display_project_name(&p.name);
            let pct = if total_cost > 0.0 { p.cost / total_cost * 100.0 } else { 0.0 };
            let (bf, be) = smooth_bar(p.cost, max_project_cost, bar_w);
            let recency = format_ago(p.last_used).replace(" ago", "");

            Row::new(vec![
                Cell::from(Span::styled(pname, Style::default().fg(FG_MUTED))),
                Cell::from(Line::from(vec![
                    Span::styled(bf, Style::default().fg(FG_MUTED)),
                    Span::styled(be, Style::default().fg(FG_FAINT)),
                ])),
                Cell::from(Span::styled(pricing::format_cost(p.cost), Style::default().fg(FG_MUTED))),
                Cell::from(Span::styled(format!("{:.0}%", pct), Style::default().fg(FG_FAINT))),
                Cell::from(Span::styled(p.session_count.to_string(), Style::default().fg(FG_FAINT))),
                Cell::from(Span::styled(recency, Style::default().fg(FG_FAINT))),
            ])
        }).collect();

        let proj_widths = [
            Constraint::Min(15),          // PROJECT
            Constraint::Length(bar_w as u16), // bar
            Constraint::Length(9),        // COST
            Constraint::Length(5),        // %
            Constraint::Length(4),        // SESS
            Constraint::Length(6),        // LAST
        ];

        let proj_table = Table::new(proj_rows, proj_widths)
            .header(proj_header)
            .column_spacing(1);
        frame.render_widget(proj_table, project_area);
    }

    // ── Help bar ──
    frame.render_widget(Paragraph::new(help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("enter", "detail"),
        ("d", "cc"),
        ("c", "cursor"),
        ("h", "history"),
        ("q", "quit"),
    ])), chunks[12]);
}

// ════════════════════════════════════════════════════════════════════════
//  Unified detail view (v4): health first, conversation on demand
// ════════════════════════════════════════════════════════════════════════

fn render_detail(
    frame: &mut ratatui::Frame,
    store: &Store,
    config: &Config,
    state: &mut DashboardState,
    live_sessions: &HashMap<String, bool>,
) {
    let area = frame.area();
    let w = area.width;
    let detail = match &state.detail {
        Some(d) => d,
        None => return,
    };

    let sessions = store.sessions_by_time();
    let meta = sessions.iter().find(|s| s.session_id == detail.session_id);
    let analysis = store.analyze_session(&detail.session_id);
    let is_live = live_sessions.get(&detail.session_id).copied().unwrap_or(false);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),  // Layer 1: header
            Constraint::Length(1),  // blank
            Constraint::Length(3),  // Layer 2: health panel
            Constraint::Length(1),  // blank
            Constraint::Min(4),    // Layer 3: context growth chart
            Constraint::Length(1),  // help bar
        ])
        .split(area);

    // ── Layer 1: Header ──
    if let Some(meta) = meta {
        let cost = detail.timeline.total_cost;
        let dur = detail.timeline.duration_minutes;
        let dur_str = if dur >= 60 { format!("{}h{:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };

        let ceiling = store.session_meta(&detail.session_id).and_then(|m| m.context_token_limit);
        let health = if let Some(ref a) = analysis {
            let status = analysis::health_status(a, ceiling, is_live, config.context_warn_pct, config.context_danger_pct);
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
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        // ── Layer 2: Health panel ──
        let mut health_lines: Vec<Line> = Vec::new();

        if let Some(ref a) = analysis {
            let (ctx_pct, _) = if let Some(ceil) = ceiling {
                let pct = (a.context_current as f64 / ceil as f64 * 100.0).min(100.0);
                (pct, format!("{}/{}", compact(a.context_current), compact(ceil)))
            } else {
                let pct = if a.context_peak > 0 { (a.context_current as f64 / a.context_peak as f64 * 100.0).min(100.0) } else { 0.0 };
                (pct, compact(a.context_current))
            };

            // Context bullet bar
            let bar_w = (w as usize).saturating_sub(55).max(10);
            let (bf, be) = smooth_bar(ctx_pct, 100.0, bar_w);
            let color = ctx_color(ctx_pct);

            health_lines.push(Line::from(vec![
                Span::styled("   ctx  ", Style::default().fg(FG_FAINT)),
                Span::styled(bf, Style::default().fg(color)),
                Span::styled(be, Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {:.0}%", ctx_pct), Style::default().fg(color).bold()),
                Span::styled(format!("   {} > {}", compact(a.context_initial), compact(a.context_current)),
                    Style::default().fg(FG_MUTED)),
                Span::styled(format!("   {:.1}x growth", a.context_growth), Style::default().fg(FG_FAINT)),
                Span::styled(format!("   cache {:.0}%", a.cache_hit_rate * 100.0), Style::default().fg(FG_FAINT)),
            ]));

            // Cost breakdown
            let cb = &a.cost_breakdown;
            health_lines.push(Line::from(vec![
                Span::styled(format!("   cost out {}   in {}   cache-r {}   cache-w {}",
                    pricing::format_cost(cb.output), pricing::format_cost(cb.input),
                    pricing::format_cost(cb.cache_read), pricing::format_cost(cb.cache_write)),
                    Style::default().fg(FG_MUTED)),
            ]));

            // Cost breakdown segmented bar
            let cost_bar_w = (w as usize).saturating_sub(10).max(10);
            let segments: Vec<(&str, f64, Color)> = vec![
                ("out", cb.output, ACCENT),
                ("cr", cb.cache_read, FG_MUTED),
                ("cw", cb.cache_write, FG_FAINT),
                ("in", cb.input, PURPLE),
            ];
            let seg_bar = segmented_bar(&segments, cost_bar_w);
            let mut seg_line: Vec<Span> = vec![Span::styled("        ", Style::default())];
            seg_line.extend(seg_bar);

            // Tool summary on same line
            let top_tools: Vec<String> = meta.tools_used.iter().take(8)
                .map(|t| { let c = meta.tool_counts.get(t).unwrap_or(&0); format!("{}({})", t, c) })
                .collect();
            if !top_tools.is_empty() {
                // Replace seg bar line with tools
                health_lines.push(Line::from(vec![
                    Span::styled("   tools ", Style::default().fg(FG_FAINT)),
                    Span::styled(top_tools.join("  "), Style::default().fg(FG_MUTED)),
                    if meta.agent_spawns > 0 {
                        Span::styled(format!("   {} agents spawned", meta.agent_spawns), Style::default().fg(PURPLE))
                    } else { Span::raw("") },
                ]));
            } else {
                health_lines.push(Line::from(seg_line));
            }
        }

        while health_lines.len() < 3 { health_lines.push(Line::from(Span::raw(""))); }
        frame.render_widget(Paragraph::new(health_lines), chunks[2]);
    }

    // ── Layer 3: Context growth chart ──
    let turns = &detail.timeline.turns;
    let bar_w = (w as usize).saturating_sub(40).max(10);
    let avg_cost = detail.timeline.avg_cost_per_turn;
    let spike_threshold = avg_cost * 2.5;

    // Select notable turns
    let thresholds = [25.0, 50.0, 75.0, 85.0];
    let mut last_crossed: Option<usize> = None;
    let mut notable_indices: Vec<usize> = Vec::new();

    for (i, turn) in turns.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == turns.len() - 1;
        let current_threshold = thresholds.iter().rposition(|&t| turn.context_pct >= t);
        let crossed_new = current_threshold != last_crossed;
        if crossed_new { last_crossed = current_threshold; }

        let is_notable = is_first || is_last || turn.is_compaction
            || (spike_threshold > 0.0 && turn.cost > spike_threshold) || crossed_new;
        if is_notable {
            notable_indices.push(i);
        }
    }

    let mut timeline_lines: Vec<Line> = Vec::new();
    timeline_lines.push(Line::from(Span::styled(
        "   CONTEXT GROWTH", Style::default().fg(ACCENT).bold(),
    )));

    let mut prev_time: Option<chrono::DateTime<chrono::Utc>> = None;
    for &idx in &notable_indices {
        let turn = &turns[idx];
        let is_first = idx == 0;
        let is_last = idx == turns.len() - 1;

        // Bug fix #3: show delta from previous notable turn, not elapsed from start
        let delta_str = if is_first {
            "  0m".to_string()
        } else if let Some(prev) = prev_time {
            let delta = (turn.timestamp - prev).num_minutes();
            if delta >= 60 { format!("{:>3}h", delta / 60) } else { format!("{:>3}m", delta) }
        } else {
            "  0m".to_string()
        };
        prev_time = Some(turn.timestamp);

        let filled = ((turn.context_pct / 100.0) * bar_w as f64).round() as usize;
        let bar_filled: String = "\u{2588}".repeat(filled);
        let bar_empty: String = "\u{2591}".repeat(bar_w.saturating_sub(filled));
        let bar_color = ctx_color(turn.context_pct);

        let event_label = if is_first { "started".to_string() }
            else if turn.is_compaction { "\u{2193} compacted".to_string() }
            else if is_last { "current".to_string() }
            else if turn.context_pct > 85.0 { "\u{26a0} near limit".to_string() }
            else if spike_threshold > 0.0 && turn.cost > spike_threshold {
                format!("cost spike {}", pricing::format_cost(turn.cost))
            }
            else { String::new() };
        let event_color = if turn.is_compaction { YELLOW }
            else if turn.context_pct > 85.0 { RED }
            else if spike_threshold > 0.0 && turn.cost > spike_threshold { YELLOW }
            else if is_last { ACCENT }
            else { FG_FAINT };

        timeline_lines.push(Line::from(vec![
            Span::styled(format!("   {} ", delta_str), Style::default().fg(FG_FAINT)),
            Span::styled(bar_filled, Style::default().fg(bar_color)),
            Span::styled(bar_empty, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>5}", compact(turn.context_size)), Style::default().fg(FG_FAINT)),
            if !event_label.is_empty() {
                Span::styled(format!("   {}", event_label), Style::default().fg(event_color))
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
            Span::styled(format!("   peak {} at turn {}", pricing::format_cost(peak_cost), peak_idx + 1), Style::default().fg(FG_MUTED)),
        ]));
    }

    // Activity strip
    if turns.len() >= 2 {
        let start = turns.first().unwrap().timestamp;
        let end = turns.last().unwrap().timestamp;
        let total_minutes = (end - start).num_minutes().max(1);
        let strip_w = (w as usize).saturating_sub(20).clamp(10, 40);

        let mut slots = vec![false; strip_w];
        for t in turns {
            let offset = (t.timestamp - start).num_minutes();
            let slot = ((offset as f64 / total_minutes as f64) * (strip_w - 1) as f64).round() as usize;
            if slot < strip_w { slots[slot] = true; }
        }

        let strip = density_strip(&slots);
        let start_str = start.format("%H:%M").to_string();
        let end_str = end.format("%H:%M").to_string();

        timeline_lines.push(Line::from(Span::raw("")));
        timeline_lines.push(Line::from(vec![
            Span::styled("     ", Style::default()),
            Span::styled(strip, Style::default().fg(FG_MUTED)),
            Span::styled("   activity pattern", Style::default().fg(FG_FAINT)),
        ]));
        timeline_lines.push(Line::from(vec![
            Span::styled(format!("     {}", start_str), Style::default().fg(FG_FAINT)),
            Span::styled(
                " ".repeat(strip_w.saturating_sub(start_str.len() + end_str.len()).max(1)),
                Style::default(),
            ),
            Span::styled(end_str, Style::default().fg(FG_FAINT)),
        ]));
    }

    frame.render_widget(Paragraph::new(timeline_lines), chunks[4]);

    // ── Help bar ──
    let help = help_bar(&[("esc", "back"), ("\u{2191}\u{2193}", "scroll"), ("q", "quit")]);
    frame.render_widget(Paragraph::new(help), chunks[5]);
}
