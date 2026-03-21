use crate::config::Config;
use crate::parser::Source;
use crate::pricing;
use crate::store::{Store, SessionTimeline};
use super::widgets::*;
use chrono::Timelike;
use ratatui::prelude::*;
use ratatui::widgets::*;

#[derive(PartialEq, Clone, Copy)]
pub enum FocusZone { ActiveSessions, Projects }

impl Default for FocusZone {
    fn default() -> Self { FocusZone::Projects }
}

#[derive(Default)]
pub struct DashboardState {
    pub focus: FocusZone,
    pub active_cursor: usize,
    pub project_cursor: usize,
    pub project_scroll: usize,
    pub detail: Option<SessionDetailView>,
    pub cached_active_ids: Vec<String>,
    pub cached_project_ids: Vec<String>,
}

pub struct SessionDetailView {
    pub session_id: String,
    pub timeline: SessionTimeline,
    pub scroll: usize,
}

impl DashboardState {
    pub fn move_up(&mut self) {
        if let Some(ref mut d) = self.detail {
            d.scroll = d.scroll.saturating_sub(1);
            return;
        }
        match self.focus {
            FocusZone::ActiveSessions => {
                self.active_cursor = self.active_cursor.saturating_sub(1);
            }
            FocusZone::Projects => {
                self.project_cursor = self.project_cursor.saturating_sub(1);
                if self.project_cursor < self.project_scroll {
                    self.project_scroll = self.project_cursor;
                }
            }
        }
    }

    pub fn move_down(&mut self, active_count: usize, _project_count: usize) {
        if let Some(ref mut d) = self.detail {
            d.scroll += 1;
            return;
        }
        match self.focus {
            FocusZone::ActiveSessions => {
                if active_count > 0 && self.active_cursor + 1 < active_count {
                    self.active_cursor += 1;
                }
            }
            FocusZone::Projects => {
                if !self.cached_project_ids.is_empty()
                    && self.project_cursor + 1 < self.cached_project_ids.len()
                {
                    self.project_cursor += 1;
                }
            }
        }
    }

    pub fn switch_focus(&mut self, active_count: usize) {
        if self.detail.is_some() { return; }
        match self.focus {
            FocusZone::ActiveSessions => { self.focus = FocusZone::Projects; }
            FocusZone::Projects => {
                if active_count > 0 { self.focus = FocusZone::ActiveSessions; }
            }
        }
    }

    pub fn enter(&mut self, store: &Store) {
        if self.detail.is_some() { return; }
        let session_id = match self.focus {
            FocusZone::ActiveSessions => self.cached_active_ids.get(self.active_cursor).cloned(),
            FocusZone::Projects => self.cached_project_ids.get(self.project_cursor).cloned(),
        };
        if let Some(id) = session_id {
            if let Some(timeline) = store.session_timeline(&id) {
                self.detail = Some(SessionDetailView {
                    session_id: id,
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
//  Overview: ticker + live sessions + project groups
// ════════════════════════════════════════════════════════════════════════

fn render_main(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut DashboardState) {
    let area = frame.area();
    let w = area.width;

    let today = store.today();
    let yesterday = store.yesterday();
    let week = store.this_week();

    let active = store.active_sessions(24);
    state.cached_active_ids = active.iter().map(|(m, _)| m.session_id.clone()).collect();

    let project_groups = store.today_by_project();
    state.cached_project_ids = project_groups.iter()
        .flat_map(|g| g.sessions.iter().cloned())
        .collect();

    if !state.cached_active_ids.is_empty() {
        state.active_cursor = state.active_cursor.min(state.cached_active_ids.len() - 1);
    }
    if !state.cached_project_ids.is_empty() {
        state.project_cursor = state.project_cursor.min(state.cached_project_ids.len() - 1);
    }

    let active_lines = if active.is_empty() { 1u16 } else { 1 + active.len() as u16 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(active_lines),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // Zone 1: Ticker
    let delta_pct = if yesterday.cost > 0.0 {
        ((today.cost - yesterday.cost) / yesterday.cost * 100.0) as i64
    } else { 0 };
    let delta_color = if delta_pct.unsigned_abs() > 50 { RED }
        else if delta_pct.unsigned_abs() > 20 { YELLOW }
        else { FG_FAINT };

    let hours_elapsed = {
        let now = chrono::Utc::now();
        (now.time().hour() as f64 + now.time().minute() as f64 / 60.0).max(0.1)
    };
    let burn_rate = today.cost / hours_elapsed;

    let spark_data = store.sessions_per_day(7);
    let spark_str = spark(&spark_data);
    let streak = store.streak_days();

    let cache_rate = store.today_cache_rate();
    let waste = store.today_waste();
    let waste_pct = if today.cost > 0.0 { waste / today.cost } else { 0.0 };
    let waste_color = if waste_pct > 0.20 { RED }
        else if waste_pct > 0.10 { YELLOW }
        else { FG_FAINT };

    let ticker = Line::from(vec![
        Span::styled("   crux", Style::default().fg(ACCENT).bold()),
        Span::styled(format!("  {}", pricing::format_cost(today.cost)), Style::default().fg(FG).bold()),
        Span::styled(format!(" {:+}%", delta_pct), Style::default().fg(delta_color)),
        Span::styled(format!("  {}/hr", pricing::format_cost(burn_rate)), Style::default().fg(FG_FAINT)),
        Span::styled("  ", Style::default()),
        Span::styled(spark_str, Style::default().fg(ACCENT)),
        Span::styled(" 7d", Style::default().fg(FG_FAINT)),
        Span::styled(format!("  streak {}d", streak), Style::default().fg(ACCENT)),
        Span::styled("      ", Style::default()),
        Span::styled(format!("cache {:.0}%", cache_rate * 100.0), Style::default().fg(FG_MUTED)),
        Span::styled(format!("  waste {}", pricing::format_cost(waste)), Style::default().fg(waste_color)),
        budget_span(config, &today, &week),
    ]);
    frame.render_widget(Paragraph::new(ticker), chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // Legend bar
    let today_date = chrono::Utc::now().date_naive();
    let mut seen_models: Vec<(String, &'static str)> = Vec::new();
    let mut seen_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for r in store.records_iter() {
        if r.timestamp.date_naive() == today_date {
            let (display, sym) = model_display(&r.model);
            if seen_set.insert(display.clone()) {
                seen_models.push((display, sym));
            }
        }
    }

    let mut legend_spans: Vec<Span> = vec![Span::styled("   ", Style::default())];
    for (display, sym) in &seen_models {
        let sym_color = if *sym == "\u{25c6}" { PURPLE }
            else if *sym == "\u{25c7}" { ACCENT }
            else { ACCENT2 };
        legend_spans.push(Span::styled(*sym, Style::default().fg(sym_color)));
        legend_spans.push(Span::styled(format!(" {display}  "), Style::default().fg(FG_MUTED)));
    }
    legend_spans.push(Span::styled("         ", Style::default()));
    legend_spans.push(Span::styled("A", Style::default().fg(GREEN)));
    legend_spans.push(Span::styled(" ", Style::default()));
    legend_spans.push(Span::styled("B", Style::default().fg(ACCENT)));
    legend_spans.push(Span::styled(" ", Style::default()));
    legend_spans.push(Span::styled("C", Style::default().fg(YELLOW)));
    legend_spans.push(Span::styled(" ", Style::default()));
    legend_spans.push(Span::styled("D", Style::default().fg(RED)));
    legend_spans.push(Span::styled("    ", Style::default()));
    legend_spans.push(Span::styled("\u{25cf}", Style::default().fg(ACCENT2)));
    legend_spans.push(Span::styled(" cc  ", Style::default().fg(FG_MUTED)));
    legend_spans.push(Span::styled("\u{25cf}", Style::default().fg(BLUE)));
    legend_spans.push(Span::styled(" cursor", Style::default().fg(FG_MUTED)));
    frame.render_widget(Paragraph::new(Line::from(legend_spans)), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // Zone 2: LIVE
    let is_active_focused = matches!(state.focus, FocusZone::ActiveSessions);
    let mut live_lines: Vec<Line> = Vec::new();

    if active.is_empty() {
        live_lines.push(Line::from(Span::styled(
            "   LIVE  no active sessions", Style::default().fg(FG_FAINT),
        )));
    } else {
        live_lines.push(Line::from(vec![
            Span::styled("   LIVE", Style::default().fg(GREEN).bold()),
            Span::styled(format!("  {} sessions", active.len()), Style::default().fg(FG_FAINT)),
        ]));

        for (i, (meta, analysis)) in active.iter().enumerate() {
            let selected = is_active_focused && i == state.active_cursor;
            let source_color = if meta.source == Source::ClaudeCode { ACCENT2 } else { BLUE };
            let raw_model = store.session_model(&meta.session_id);
            let sym = model_symbol(&raw_model);
            let sym_color = if sym == "\u{25c6}" { PURPLE } else if sym == "\u{25c7}" { ACCENT } else { ACCENT2 };

            let topic = truncate(&meta.first_message, 20);
            let dur = meta.duration_minutes();
            let dur_str = if dur >= 60 { format!("{}h{:02}", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };
            let cost_str = pricing::format_cost(analysis.total_cost);

            let ctx_pct = (analysis.context_current as f64 / 167_000.0 * 100.0).min(100.0);
            let (bar_f, bar_e) = smooth_bar(ctx_pct, 100.0, 3);
            let ctx_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { GREEN };

            let grade = analysis.grade_letter();
            let grade_color = match grade { "A" => GREEN, "B" => ACCENT, "C" => YELLOW, _ => RED };

            let mut tool_pairs: Vec<(&String, &usize)> = meta.tool_counts.iter().collect();
            tool_pairs.sort_by(|a, b| b.1.cmp(a.1));
            let tool_str: String = tool_pairs.iter().take(3)
                .map(|(name, _)| shorten_tool(name))
                .collect::<Vec<_>>()
                .join(" ");

            let cursor_char = if selected { "\u{25b8}" } else { " " };
            let cost_color = if selected { ACCENT } else { FG_MUTED };

            live_lines.push(Line::from(vec![
                Span::styled(format!("  {cursor_char} "), Style::default().fg(if selected { ACCENT } else { FG_FAINT })),
                Span::styled("\u{25cf}", Style::default().fg(source_color)),
                Span::styled(format!(" {sym}"), Style::default().fg(sym_color)),
                Span::styled(format!(" {topic:<20}"), Style::default().fg(if selected { FG } else { FG_MUTED })),
                Span::styled(format!(" {:>5}", dur_str), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {cost_str}"), Style::default().fg(cost_color)),
                Span::styled(format!("  {bar_f}{bar_e}"), Style::default().fg(ctx_color)),
                Span::styled(format!(" {:>3.0}%", ctx_pct), Style::default().fg(ctx_color)),
                Span::styled(format!("  {grade}"), Style::default().fg(grade_color)),
                Span::styled(format!("  {tool_str}"), Style::default().fg(FG_FAINT)),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(live_lines), chunks[4]);
    frame.render_widget(Paragraph::new(dashed_divider(w)), chunks[5]);

    // Zone 3: Project groups
    let is_project_focused = matches!(state.focus, FocusZone::Projects);
    let project_area = chunks[6];
    let visible_height = project_area.height as usize;

    if project_groups.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "   no sessions today", Style::default().fg(FG_FAINT),
            ))),
            project_area,
        );
    } else {
        let mut lines: Vec<Line> = Vec::new();
        let mut line_to_session: Vec<Option<usize>> = Vec::new();
        let mut flat_idx = 0usize;

        for (gi, group) in project_groups.iter().enumerate() {
            if gi > 0 {
                lines.push(Line::default());
                line_to_session.push(None);
            }

            let display_name = display_project_name(&group.name);
            let right = format!("{} \u{00b7} {} sess", pricing::format_cost(group.cost), group.sessions.len());
            let pad = (w as usize).saturating_sub(display_name.len() + right.len() + 6).max(1);
            let header = Line::from(vec![
                Span::styled(format!("   {display_name}"), Style::default().fg(ACCENT).bold()),
                Span::styled(" ".repeat(pad), Style::default()),
                Span::styled(right, Style::default().fg(FG_MUTED)),
            ]);
            lines.push(header);
            line_to_session.push(None);

            for session_id in &group.sessions {
                let meta = store.session_meta(session_id);
                let analysis = store.analyze_session(session_id);

                if let (Some(meta), Some(analysis)) = (meta, analysis) {
                    let selected = is_project_focused && flat_idx == state.project_cursor;
                    let source_color = if meta.source == Source::ClaudeCode { ACCENT2 } else { BLUE };
                    let raw_model = store.session_model(session_id);
                    let sym = model_symbol(&raw_model);
                    let sym_color = if sym == "\u{25c6}" { PURPLE } else if sym == "\u{25c7}" { ACCENT } else { ACCENT2 };

                    let topic = truncate(&meta.first_message, 20);
                    let dur = meta.duration_minutes();
                    let dur_str = if dur >= 60 { format!("{}h{:02}", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };
                    let cost_str = pricing::format_cost(analysis.total_cost);
                    let ctx_pct = (analysis.context_current as f64 / 167_000.0 * 100.0).min(100.0);

                    let grade = analysis.grade_letter();
                    let grade_color = match grade { "A" => GREEN, "B" => ACCENT, "C" => YELLOW, _ => RED };

                    let cursor_char = if selected { "\u{25b8}" } else { " " };
                    let cost_color = if selected { ACCENT } else { FG_MUTED };

                    lines.push(Line::from(vec![
                        Span::styled(format!("    {cursor_char} "), Style::default().fg(if selected { ACCENT } else { FG_FAINT })),
                        Span::styled("\u{25cf}", Style::default().fg(source_color)),
                        Span::styled(format!(" {sym}"), Style::default().fg(sym_color)),
                        Span::styled(format!(" {topic:<20}"), Style::default().fg(if selected { FG } else { FG_MUTED })),
                        Span::styled(format!(" {:>5}", dur_str), Style::default().fg(FG_FAINT)),
                        Span::styled(format!("  {cost_str}"), Style::default().fg(cost_color)),
                        Span::styled(format!("  {:>3.0}%", ctx_pct), Style::default().fg(FG_MUTED)),
                        Span::styled(format!("  {grade}"), Style::default().fg(grade_color)),
                    ]));
                    line_to_session.push(Some(flat_idx));
                }
                flat_idx += 1;
            }
        }

        let cursor_line = line_to_session.iter()
            .position(|idx| *idx == Some(state.project_cursor))
            .unwrap_or(0);
        if cursor_line < state.project_scroll {
            state.project_scroll = cursor_line;
        } else if cursor_line >= state.project_scroll + visible_height {
            state.project_scroll = cursor_line.saturating_sub(visible_height) + 1;
        }

        let visible: Vec<Line> = lines.into_iter()
            .skip(state.project_scroll)
            .take(visible_height)
            .collect();
        frame.render_widget(Paragraph::new(visible), project_area);
    }

    // Bottom
    frame.render_widget(Paragraph::new(divider(w)), chunks[7]);
    frame.render_widget(Paragraph::new(help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("tab", "zone"),
        ("enter", "detail"),
        ("d", "cc"),
        ("c", "cursor"),
        ("h", "history"),
        ("q", "quit"),
    ])), chunks[8]);
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
