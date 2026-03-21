use crate::config::Config;
use crate::parser::Source;
use crate::parser::conversation::{SessionStatus, SessionMode};
use crate::pricing;
use crate::store::{Store, SessionTimeline};
use super::widgets::*;
use chrono::Timelike;
use ratatui::prelude::*;
use ratatui::widgets::*;

#[derive(PartialEq, Clone, Copy, Default)]
pub enum FocusZone {
    #[default]
    ActiveSessions,
    CcPane,
    CuPane,
}

#[derive(Default)]
pub struct DashboardState {
    pub focus: FocusZone,
    pub active_cursor: usize,
    pub cc_cursor: usize,
    pub cu_cursor: usize,
    pub detail: Option<SessionDetailView>,
    cached_active_ids: Vec<String>,
}

pub struct SessionDetailView {
    pub session_id: String,
    pub timeline: SessionTimeline,
    pub scroll: usize,
}

impl DashboardState {
    pub fn move_up(&mut self) {
        if let Some(ref mut detail) = self.detail {
            detail.scroll = detail.scroll.saturating_sub(1);
            return;
        }
        match self.focus {
            FocusZone::ActiveSessions => { self.active_cursor = self.active_cursor.saturating_sub(1); }
            FocusZone::CcPane => { self.cc_cursor = self.cc_cursor.saturating_sub(1); }
            FocusZone::CuPane => { self.cu_cursor = self.cu_cursor.saturating_sub(1); }
        }
    }

    pub fn move_down(&mut self, active_count: usize, _project_count: usize) {
        if let Some(ref mut detail) = self.detail {
            detail.scroll += 1;
            return;
        }
        match self.focus {
            FocusZone::ActiveSessions => {
                if active_count > 0 && self.active_cursor + 1 < active_count { self.active_cursor += 1; }
            }
            FocusZone::CcPane => { self.cc_cursor += 1; }
            FocusZone::CuPane => { self.cu_cursor += 1; }
        }
    }

    pub fn switch_focus(&mut self, active_count: usize) {
        if self.detail.is_some() { return; }
        self.focus = match self.focus {
            FocusZone::ActiveSessions => FocusZone::CcPane,
            FocusZone::CcPane => FocusZone::CuPane,
            FocusZone::CuPane => {
                if active_count > 0 { FocusZone::ActiveSessions } else { FocusZone::CcPane }
            }
        };
    }

    pub fn enter(&mut self, store: &Store) {
        if self.detail.is_some() { return; }
        if self.focus != FocusZone::ActiveSessions { return; }
        if let Some(session_id) = self.cached_active_ids.get(self.active_cursor) {
            if let Some(timeline) = store.session_timeline(session_id) {
                self.detail = Some(SessionDetailView {
                    session_id: session_id.clone(),
                    timeline,
                    scroll: 0,
                });
            }
        }
    }

    pub fn back(&mut self) -> bool {
        if self.detail.is_some() { self.detail = None; true } else { false }
    }
}

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut DashboardState) {
    if state.detail.is_some() {
        render_detail(frame, store, config, state);
    } else {
        render_main(frame, store, config, state);
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Overview: ticker + active sessions + split CC/Cursor panes
// ════════════════════════════════════════════════════════════════════════

fn render_main(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut DashboardState) {
    let area = frame.area();
    let w = area.width;

    let active = store.active_sessions(24);
    let active_count = active.len();
    state.cached_active_ids = active.iter().map(|(m, _)| m.session_id.clone()).collect();

    let today = store.today();
    let yesterday = store.yesterday();
    let week = store.this_week();
    // Active zone: ~4 lines per session + header, capped
    let active_height = if active_count > 0 {
        (active_count as u16 * 4 + 1).min(14)
    } else { 1 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),              // ticker
            Constraint::Length(1),              // divider
            Constraint::Length(active_height),  // active sessions
            Constraint::Length(1),              // divider
            Constraint::Min(6),                // split panes
            Constraint::Length(1),              // divider
            Constraint::Length(1),              // help
        ])
        .split(area);

    // ── Ticker bar ──
    let today_delta = if yesterday.cost > 0.0 {
        ((today.cost - yesterday.cost) / yesterday.cost * 100.0) as i64
    } else { 0 };
    let delta_str = if today_delta > 0 { format!("+{}%", today_delta) }
        else if today_delta < 0 { format!("{}%", today_delta) }
        else { "flat".to_string() };
    let delta_color = if today_delta > 50 { RED } else if today_delta > 20 { YELLOW } else { FG_FAINT };

    let hours_elapsed = {
        let now = chrono::Utc::now();
        (now.time().hour() as f64 + now.time().minute() as f64 / 60.0).max(0.1)
    };
    let burn_rate = today.cost / hours_elapsed;

    let sessions_spark = store.sessions_per_day(7);
    let spark_str = spark(&sessions_spark);
    let streak = store.streak_days();

    let right_info = format!(
        "{}  {}  {}/hr  {} 7d  streak {}d",
        pricing::format_cost(today.cost),
        delta_str,
        pricing::format_cost(burn_rate),
        spark_str,
        streak,
    );
    let pad = (w as usize).saturating_sub(8 + right_info.len()).max(1);

    let title = Line::from(vec![
        Span::styled("   crux", Style::default().fg(ACCENT).bold()),
        Span::styled(" ".repeat(pad), Style::default()),
        Span::styled(pricing::format_cost(today.cost), Style::default().fg(FG).bold()),
        Span::styled("  ", Style::default()),
        Span::styled(delta_str, Style::default().fg(delta_color)),
        Span::styled("  ", Style::default()),
        Span::styled(format!("{}/hr", pricing::format_cost(burn_rate)), Style::default().fg(FG_FAINT)),
        Span::styled("  ", Style::default()),
        Span::styled(spark_str, Style::default().fg(ACCENT)),
        Span::styled(" 7d  ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("streak {}d", streak), Style::default().fg(ACCENT)),
        budget_span(config, &today, &week),
    ]);
    frame.render_widget(Paragraph::new(title), chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Active Sessions ──
    if active_count > 0 {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("   LIVE", Style::default().fg(GREEN).bold()),
                Span::styled(
                    format!("  {}", if active_count == 1 { "1 session".to_string() } else { format!("{} sessions", active_count) }),
                    Style::default().fg(FG_FAINT),
                ),
            ]),
        ];

        for (i, (meta, analysis)) in active.iter().take(3).enumerate() {
            let is_selected = state.focus == FocusZone::ActiveSessions && i == state.active_cursor;
            let cost = analysis.total_cost;
            let ctx_pct = (analysis.context_current as f64 / 167_000.0 * 100.0).min(100.0);
            let bar_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { GREEN };

            let cursor_char = if is_selected { "\u{25b8}" } else { " " };
            let fg = if is_selected { FG } else { FG_MUTED };
            let cost_fg = if is_selected { ACCENT } else { FG_MUTED };

            let dur = meta.duration_minutes();
            let dur_str = if dur >= 60 { format!("{}h{:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };

            let tool_calls: usize = meta.tool_counts.values().sum();
            let tool_heavy = tool_calls as f64 / meta.message_count.max(1) as f64 > 0.5;
            let badge = if meta.agent_spawns > 2 { ("agentic", PURPLE) }
                else if tool_heavy { ("tools", FG_FAINT) }
                else { ("chat", FG_FAINT) };

            let source_badge = match meta.source {
                Source::ClaudeCode => ("\u{25cf}", ACCENT2),
                Source::Cursor => ("\u{25cf}", BLUE),
            };

            let name_w = (w as usize).saturating_sub(70).max(8);
            let topic = truncate(&meta.first_message, name_w);
            let project_name = display_project_name(&meta.project);

            // Line 1: source + topic + metrics
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", cursor_char), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
                Span::styled(source_badge.0, Style::default().fg(source_badge.1)),
                Span::styled(format!(" {:<width$}", topic, width = name_w), Style::default().fg(fg)),
                Span::styled(format!(" {}", dur_str), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {}", pricing::format_cost(cost)), Style::default().fg(cost_fg)),
                Span::styled(format!("  {}t", meta.user_count), Style::default().fg(FG_FAINT)),
                if meta.agent_spawns > 0 {
                    Span::styled(format!("  {}ag", meta.agent_spawns), Style::default().fg(PURPLE))
                } else { Span::raw("") },
                Span::styled(format!("  {}", badge.0), Style::default().fg(badge.1)),
            ]));

            // Line 2: context fill LineGauge (text-based since we can't nest widgets easily)
            let bar_w = (w as usize).saturating_sub(40).max(10);
            let (bar_f, bar_e) = smooth_bar(ctx_pct, 100.0, bar_w);
            lines.push(Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(bar_f, Style::default().fg(bar_color)),
                Span::styled(bar_e, Style::default().fg(FG_FAINT)),
                Span::styled(format!(" {:.0}%", ctx_pct), Style::default().fg(bar_color).bold()),
                Span::styled(format!("  {:.1}x", analysis.context_growth), Style::default().fg(FG_MUTED)),
                Span::styled(format!("  cache {:.0}%", analysis.cache_hit_rate * 100.0), Style::default().fg(FG_FAINT)),
                if analysis.compaction_count > 0 {
                    Span::styled(format!("  {} compacted", analysis.compaction_count), Style::default().fg(YELLOW))
                } else { Span::raw("") },
            ]));

            // Line 3: context trajectory sparkline (Braille-like using block chars)
            if let Some(timeline) = store.session_timeline(&meta.session_id) {
                let ctx_values: Vec<f64> = timeline.turns.iter().map(|t| t.context_pct).collect();
                let ctx_spark = spark(&ctx_values);
                let grade = analysis.grade_letter();
                let grade_c = match grade {
                    "A" => GREEN, "B" => ACCENT, "C" => YELLOW, _ => RED,
                };
                lines.push(Line::from(vec![
                    Span::styled("     ctx ", Style::default().fg(FG_FAINT)),
                    Span::styled(ctx_spark, Style::default().fg(bar_color)),
                    Span::styled(format!("  {} \u{2192} {}", compact(analysis.context_initial), compact(analysis.context_current)),
                        Style::default().fg(FG_FAINT)),
                    Span::styled(format!("  {}", grade), Style::default().fg(grade_c).bold()),
                    Span::styled(format!("  {}", project_name), Style::default().fg(FG_FAINT)),
                ]));
            }

            if i + 1 < active_count.min(3) {
                lines.push(Line::from(Span::raw("")));
            }
        }

        frame.render_widget(Paragraph::new(lines), chunks[2]);
    } else {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("   no active sessions", Style::default().fg(FG_FAINT)),
            ])),
            chunks[2],
        );
    }
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // ── Split panes: CC left, Cursor right ──
    let pane_area = chunks[4];
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(pane_area);

    render_cc_pane(frame, store, state, panes[0]);
    render_cu_pane(frame, store, state, panes[1]);

    frame.render_widget(Paragraph::new(divider(w)), chunks[5]);

    let help = help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("tab", "switch"),
        ("enter", "detail"),
        ("d", "claude code"),
        ("c", "cursor"),
        ("h", "history"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[6]);
}

// ── Claude Code pane ──
fn render_cc_pane(frame: &mut ratatui::Frame, store: &Store, state: &DashboardState, area: Rect) {
    let is_focused = state.focus == FocusZone::CcPane;
    let cc_today = store.today_by_source(Source::ClaudeCode);
    let cc_sessions = store.today_sessions_by_source(Source::ClaudeCode);
    let session_count = cc_sessions.len();

    let border_style = if is_focused { Style::default().fg(ACCENT2) } else { Style::default().fg(FG_FAINT) };
    let block = Block::default()
        .title(Span::styled(" Claude Code ", Style::default().fg(ACCENT2).bold()))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_type(BorderType::Rounded);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let inner_w = inner.width as usize;
    let max_rows = inner.height as usize;
    let mut lines: Vec<Line> = Vec::new();

    // Header: cost + session count
    lines.push(Line::from(vec![
        Span::styled(format!(" {}", pricing::format_cost(cc_today.cost)), Style::default().fg(FG).bold()),
        Span::styled(format!("  {} sessions", session_count), Style::default().fg(FG_FAINT)),
    ]));

    // Session rows
    for (i, session) in cc_sessions.iter().take(max_rows.saturating_sub(3)).enumerate() {
        let analysis = store.analyze_session(&session.session_id);
        let cost = analysis.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
        let ctx_pct = analysis.as_ref().map(|a| (a.context_current as f64 / 167_000.0 * 100.0).min(100.0)).unwrap_or(0.0);
        let grade = analysis.as_ref().map(|a| a.grade_letter()).unwrap_or("-");

        let dur = session.duration_minutes();
        let dur_str = if dur >= 60 { format!("{}h{:02}", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };

        let ctx_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { GREEN };
        let health_dot = "\u{25cf}";

        // Context mini sparkline
        let ctx_bar_w = 5;
        let (bf, _be) = smooth_bar(ctx_pct, 100.0, ctx_bar_w);

        let is_sel = is_focused && i == state.cc_cursor;
        let fg = if is_sel { FG } else { FG_MUTED };
        let name_w = inner_w.saturating_sub(35).max(6);
        let topic = truncate(&session.first_message, name_w);

        lines.push(Line::from(vec![
            Span::styled(format!(" {}", health_dot), Style::default().fg(ctx_color)),
            Span::styled(format!(" {:<w$}", topic, w = name_w), Style::default().fg(fg)),
            Span::styled(format!(" {:>4}", dur_str), Style::default().fg(FG_FAINT)),
            Span::styled(format!(" {}", pricing::format_cost(cost)), Style::default().fg(if is_sel { ACCENT } else { FG_FAINT })),
            Span::styled(format!(" {}", bf), Style::default().fg(ctx_color)),
            Span::styled(format!("{:>3.0}%", ctx_pct), Style::default().fg(FG_FAINT)),
            Span::styled(format!(" {}", grade), Style::default().fg(match grade { "A" => GREEN, "B" => ACCENT, "C" => YELLOW, _ => RED })),
        ]));
    }

    // KPI strip at bottom
    let cache_pct = all_time_cache_denom(store).1;

    if lines.len() < max_rows {
        lines.push(Line::from(vec![
            Span::styled(format!(" cache {:.0}%", cache_pct), Style::default().fg(if cache_pct > 60.0 { GREEN } else { YELLOW })),
            Span::styled(format!("  {} total tok", compact(cc_today.total_tokens())), Style::default().fg(FG_FAINT)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn all_time_cache_denom(store: &Store) -> (u64, f64) {
    let all = store.all_time();
    let denom = all.cache_read_tokens + all.input_tokens;
    let pct = if denom > 0 { all.cache_read_tokens as f64 / denom as f64 * 100.0 } else { 0.0 };
    (denom, pct)
}

// ── Cursor pane ──
fn render_cu_pane(frame: &mut ratatui::Frame, store: &Store, state: &DashboardState, area: Rect) {
    let is_focused = state.focus == FocusZone::CuPane;
    let cu_sessions = store.today_sessions_by_source(Source::Cursor);
    let cu_today = store.today_by_source(Source::Cursor);
    let session_count = cu_sessions.len();

    let border_style = if is_focused { Style::default().fg(BLUE) } else { Style::default().fg(FG_FAINT) };
    let block = Block::default()
        .title(Span::styled(" Cursor ", Style::default().fg(BLUE).bold()))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_type(BorderType::Rounded);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let inner_w = inner.width as usize;
    let max_rows = inner.height as usize;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(format!(" {}", pricing::format_cost(cu_today.cost)), Style::default().fg(FG).bold()),
        Span::styled(format!("  {} sessions", session_count), Style::default().fg(FG_FAINT)),
    ]));

    let stats = store.cursor_overview_stats();

    for (i, session) in cu_sessions.iter().take(max_rows.saturating_sub(3)).enumerate() {
        let (status_icon, status_color) = match session.cursor_status {
            Some(SessionStatus::Completed) => ("\u{25cf}", GREEN),
            Some(SessionStatus::Aborted) => ("\u{25cf}", RED),
            _ => ("\u{25cb}", FG_FAINT),
        };
        let status_text = match session.cursor_status {
            Some(SessionStatus::Completed) => "done",
            Some(SessionStatus::Aborted) => "abort",
            _ => "",
        };
        let mode_badge = match session.cursor_mode {
            Some(SessionMode::Agent) => "agt",
            Some(SessionMode::Chat) => "chat",
            _ => "",
        };

        let dur = session.duration_minutes();
        let dur_str = if dur >= 60 { format!("{}h{:02}", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };

        let lines_str = session.lines_added.map(|a| format!("+{}", a)).unwrap_or_default();

        let is_sel = is_focused && i == state.cu_cursor;
        let fg = if is_sel { FG } else { FG_MUTED };
        let name_w = inner_w.saturating_sub(32).max(6);
        let topic = truncate(&session.first_message, name_w);

        lines.push(Line::from(vec![
            Span::styled(format!(" {}", status_icon), Style::default().fg(status_color)),
            Span::styled(format!(" {:<w$}", topic, w = name_w), Style::default().fg(fg)),
            Span::styled(format!(" {:<4}", mode_badge), Style::default().fg(if mode_badge == "agt" { PURPLE } else { FG_FAINT })),
            Span::styled(format!("{:>4}", dur_str), Style::default().fg(FG_FAINT)),
            Span::styled(format!(" {:<5}", status_text), Style::default().fg(status_color)),
            Span::styled(format!("{:>6}", lines_str), Style::default().fg(FG_MUTED)),
        ]));
    }

    // KPI strip
    if lines.len() < max_rows {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:.0}% done", stats.completion_rate), Style::default().fg(if stats.completion_rate > 70.0 { GREEN } else { YELLOW })),
            Span::styled(format!("  {} lines shipped", compact(stats.total_lines)), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:.0}% agent", stats.agent_pct), Style::default().fg(PURPLE)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

// ════════════════════════════════════════════════════════════════════════
//  Budget span
// ════════════════════════════════════════════════════════════════════════

fn budget_span<'a>(config: &Config, today: &crate::store::Aggregation, week: &crate::store::Aggregation) -> Span<'a> {
    let (label, pct) = if let Some(budget) = config.budget_daily {
        ("daily", today.cost / budget * 100.0)
    } else if let Some(budget) = config.budget_weekly {
        ("weekly", week.cost / budget * 100.0)
    } else {
        return Span::raw("");
    };
    let color = if pct > 90.0 { RED } else if pct > 70.0 { YELLOW } else { FG_FAINT };
    Span::styled(format!("  {} budget {:.0}%", label, pct), Style::default().fg(color))
}

// ════════════════════════════════════════════════════════════════════════
//  Session detail view
// ════════════════════════════════════════════════════════════════════════

fn render_detail(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &mut DashboardState) {
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

        let grade_str = if let Some(ref a) = analysis {
            let g = a.grade_letter();
            let gc = match g { "A" => GREEN, "B" => ACCENT, "C" => YELLOW, _ => RED };
            (g, gc)
        } else { ("-", FG_FAINT) };

        let ctx_pct = analysis.as_ref().map(|a| (a.context_current as f64 / 167_000.0 * 100.0).min(100.0)).unwrap_or(0.0);
        let health = analysis.as_ref().map(|a| session_health(a, ctx_pct)).unwrap_or(("", FG_FAINT));

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
                Span::styled(format!("  {}", grade_str.0), Style::default().fg(grade_str.1).bold()),
                if detail.timeline.compaction_count > 0 {
                    Span::styled(format!("  {} compactions", detail.timeline.compaction_count), Style::default().fg(YELLOW))
                } else { Span::raw("") },
                Span::styled(format!("  {}", health.0), Style::default().fg(health.1)),
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

fn session_health(analysis: &crate::store::SessionAnalysis, ctx_pct: f64) -> (&'static str, Color) {
    if ctx_pct > 85.0 { return ("START NEW SESSION", RED); }
    if analysis.context_growth > 6.0 && analysis.output_efficiency < 0.1 {
        return ("YIELD DECLINING", RED);
    }
    if ctx_pct > 70.0 && analysis.context_growth > 4.0 { return ("SESSION AGING", YELLOW); }
    if analysis.messages_since_compaction > 30 && analysis.context_growth > 3.0 {
        return ("LONG SINCE COMPACTION", YELLOW);
    }
    if ctx_pct < 40.0 { return ("FRESH", GREEN); }
    ("OK", FG_FAINT)
}
