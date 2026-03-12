use crate::config::Config;
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
    Projects,
}

#[derive(Default)]
pub struct DashboardState {
    pub focus: FocusZone,
    pub active_cursor: usize,
    pub project_cursor: usize,
    pub project_scroll: usize,
    pub detail: Option<SessionDetailView>,
    /// Cached session IDs from the last render, so enter() indexes the same list.
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
            FocusZone::ActiveSessions => {
                self.active_cursor = self.active_cursor.saturating_sub(1);
            }
            FocusZone::Projects => {
                if self.project_cursor > 0 {
                    self.project_cursor -= 1;
                    if self.project_cursor < self.project_scroll {
                        self.project_scroll = self.project_cursor;
                    }
                }
            }
        }
    }

    pub fn move_down(&mut self, active_count: usize, project_count: usize) {
        if let Some(ref mut detail) = self.detail {
            detail.scroll += 1;
            return;
        }
        match self.focus {
            FocusZone::ActiveSessions => {
                if active_count > 0 && self.active_cursor + 1 < active_count {
                    self.active_cursor += 1;
                }
            }
            FocusZone::Projects => {
                if project_count > 0 && self.project_cursor + 1 < project_count {
                    self.project_cursor += 1;
                    if self.project_cursor >= self.project_scroll + 10 {
                        self.project_scroll = self.project_cursor.saturating_sub(9);
                    }
                }
            }
        }
    }

    pub fn switch_focus(&mut self, active_count: usize) {
        if self.detail.is_some() { return; }
        match self.focus {
            FocusZone::ActiveSessions => {
                self.focus = FocusZone::Projects;
            }
            FocusZone::Projects => {
                if active_count > 0 {
                    self.focus = FocusZone::ActiveSessions;
                }
            }
        }
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
        if self.detail.is_some() {
            self.detail = None;
            true
        } else {
            false
        }
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
//  Main dashboard
// ════════════════════════════════════════════════════════════════════════

fn render_main(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut DashboardState) {
    let area = frame.area();
    let w = area.width;

    let active = store.active_sessions(24);
    let active_count = active.len();
    state.cached_active_ids = active.iter().map(|(m, _)| m.session_id.clone()).collect();

    // Compute data upfront
    let today = store.today();
    let yesterday = store.yesterday();
    let week = store.this_week();
    let all = store.all_time();

    // Active sessions: 4 lines per session (topic, ctx bar, health, blank) + 1 header
    let active_height = if active_count > 0 {
        (active_count as u16 * 4 + 1).min(14)
    } else {
        1
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),              // title/ticker
            Constraint::Length(1),              // divider
            Constraint::Length(active_height),  // active sessions
            Constraint::Length(1),              // divider
            Constraint::Length(2),              // KPI strip + 7d sparkline
            Constraint::Length(1),              // divider
            Constraint::Length(1),              // projects header
            Constraint::Min(3),                // project list
            Constraint::Length(1),              // divider
            Constraint::Length(1),              // help bar
        ])
        .split(area);

    // ── Title ticker ──
    let today_delta = if yesterday.cost > 0.0 {
        ((today.cost - yesterday.cost) / yesterday.cost * 100.0) as i64
    } else {
        0
    };
    let delta_str = if today_delta > 0 {
        format!("+{}%", today_delta)
    } else if today_delta < 0 {
        format!("{}%", today_delta)
    } else {
        "flat".to_string()
    };
    let delta_color = if today_delta > 50 { RED } else if today_delta > 20 { YELLOW } else { FG_FAINT };

    // Burn rate: today's cost / hours elapsed today
    let hours_elapsed = {
        let now = chrono::Utc::now();
        let h = now.time().hour() as f64 + now.time().minute() as f64 / 60.0;
        h.max(0.1)
    };
    let burn_rate = today.cost / hours_elapsed;

    let right_info = format!(
        "{}  {}  {}/hr",
        pricing::format_cost(today.cost),
        delta_str,
        pricing::format_cost(burn_rate),
    );
    let pad = (w as usize).saturating_sub(8 + right_info.len()).max(1);

    let title = Line::from(vec![
        Span::styled("   crux", Style::default().fg(ACCENT).bold()),
        Span::styled(" ".repeat(pad), Style::default()),
        Span::styled(pricing::format_cost(today.cost), Style::default().fg(FG).bold()),
        Span::styled("  ", Style::default()),
        Span::styled(delta_str.clone(), Style::default().fg(delta_color)),
        Span::styled("  ", Style::default()),
        Span::styled(format!("{}/hr", pricing::format_cost(burn_rate)), Style::default().fg(FG_FAINT)),
    ]);
    frame.render_widget(Paragraph::new(title), chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Active Sessions ──
    let in_active_zone = state.focus == FocusZone::ActiveSessions;

    if active_count > 0 {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("   LIVE", Style::default().fg(Color::Rgb(120, 190, 120)).bold()),
                Span::styled(
                    format!("  {}", if active_count == 1 { "1 session".to_string() } else { format!("{} sessions", active_count) }),
                    Style::default().fg(FG_FAINT),
                ),
            ]),
        ];

        for (i, (meta, analysis)) in active.iter().take(3).enumerate() {
            let is_selected = in_active_zone && i == state.active_cursor;
            let cost = analysis.total_cost;
            let ctx_pct = (analysis.context_current as f64 / 167_000.0 * 100.0).min(100.0);
            let bar_w = 20usize;
            let bar_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { Color::Rgb(120, 190, 120) };

            let cursor_char = if is_selected { "\u{25b8}" } else { " " };
            let fg = if is_selected { FG } else { FG_MUTED };
            let cost_fg = if is_selected { ACCENT } else { FG_MUTED };

            // Duration
            let dur = meta.duration_minutes();
            let dur_str = if dur >= 60 {
                format!("{}h{:02}m", dur / 60, dur % 60)
            } else {
                format!("{}m", dur.max(1))
            };

            // Session type badge based on tool usage ratio
            let total_msgs = meta.message_count.max(1);
            let tool_calls: usize = meta.tool_counts.values().sum();
            let badge = if meta.agent_spawns > 2 {
                ("agentic", YELLOW)
            } else if tool_calls as f64 / total_msgs as f64 > 0.5 {
                ("tools", FG_FAINT)
            } else {
                ("chat", FG_FAINT)
            };

            let name_w = (w as usize).saturating_sub(68).max(8);
            let topic = truncate(&meta.first_message, name_w);

            // Line 1: topic + key metrics
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", cursor_char), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
                Span::styled(format!("{:<width$}", topic, width = name_w), Style::default().fg(fg)),
                Span::styled(format!(" {}", dur_str), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {}", pricing::format_cost(cost)), Style::default().fg(cost_fg)),
                Span::styled(format!("  {}m", meta.user_count), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {}", badge.0), Style::default().fg(badge.1)),
            ]));

            // Line 2: context bar + stats
            let (bar_f, bar_e) = smooth_bar(ctx_pct, 100.0, bar_w);
            let mut ctx_spans = vec![
                Span::styled("     ", Style::default()),
                Span::styled(bar_f, Style::default().fg(bar_color)),
                Span::styled(bar_e, Style::default().fg(FG_FAINT)),
                Span::styled(
                    format!(" {:.0}%", ctx_pct),
                    Style::default().fg(bar_color).bold(),
                ),
                Span::styled(
                    format!("  {}  {:.1}x", compact(analysis.context_current), analysis.context_growth),
                    Style::default().fg(FG_MUTED),
                ),
                Span::styled(
                    format!("  cache {:.0}%", analysis.cache_hit_rate * 100.0),
                    Style::default().fg(FG_FAINT),
                ),
            ];
            if analysis.compaction_count > 0 {
                ctx_spans.push(Span::styled(
                    format!("  {} compacted", analysis.compaction_count),
                    Style::default().fg(YELLOW),
                ));
            }
            lines.push(Line::from(ctx_spans));

            // Line 3: health verdict
            let health = session_health(analysis, ctx_pct);
            lines.push(Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(health.0, Style::default().fg(health.1)),
            ]));

            // Blank separator between sessions
            if i + 1 < active_count.min(3) {
                lines.push(Line::from(Span::raw("")));
            }
        }

        frame.render_widget(Paragraph::new(lines), chunks[2]);
    } else {
        let no_active = Paragraph::new(Line::from(vec![
            Span::styled("   no active sessions", Style::default().fg(FG_FAINT)),
        ]));
        frame.render_widget(no_active, chunks[2]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // ── KPI strip + 7d sparkline ──
    // Line 1: four KPIs
    let cache_denom = all.cache_read_tokens + all.input_tokens;
    let cache_hit = if cache_denom > 0 { all.cache_read_tokens as f64 / cache_denom as f64 } else { 0.0 };
    let cache_c = if cache_hit > 0.6 { Color::Rgb(120, 190, 120) } else if cache_hit > 0.3 { YELLOW } else { RED };

    let output_eff = if all.input_tokens > 0 { all.output_tokens as f64 / all.input_tokens as f64 } else { 0.0 };
    let eff_c = if output_eff > 0.3 { Color::Rgb(120, 190, 120) } else if output_eff > 0.1 { FG_MUTED } else { YELLOW };

    let avg_cost = store.avg_session_cost_historical();
    let streak = store.streak_days();

    let kpi_line = Line::from(vec![
        Span::styled("   cache ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.0}%", cache_hit * 100.0), Style::default().fg(cache_c).bold()),
        Span::styled("     yield ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.0}%", output_eff * 100.0), Style::default().fg(eff_c).bold()),
        Span::styled("     $/sess ", Style::default().fg(FG_FAINT)),
        Span::styled(pricing::format_cost(avg_cost), Style::default().fg(FG_MUTED)),
        Span::styled("     streak ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{}d", streak), Style::default().fg(ACCENT)),
        // Budget inline if configured
        budget_span(config, &today, &week),
    ]);

    // Line 2: 7d sparkline + week total
    let days_data = store.by_day(7);
    let sessions_spark = store.sessions_per_day(7);
    let spark_str = spark(&sessions_spark);
    let week_sessions: usize = days_data.iter().map(|d| d.session_count).sum();

    let sparkline_line = Line::from(vec![
        Span::styled("   7d ", Style::default().fg(FG_FAINT)),
        Span::styled(spark_str, Style::default().fg(ACCENT)),
        Span::styled(
            format!("  {}  {} sessions  {} today / {} this week",
                pricing::format_cost(week.cost),
                week_sessions,
                today.session_count,
                week.session_count,
            ),
            Style::default().fg(FG_MUTED),
        ),
    ]);

    frame.render_widget(Paragraph::new(vec![kpi_line, sparkline_line]), chunks[4]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[5]);

    // ── Projects header ──
    let in_project_zone = state.focus == FocusZone::Projects;
    let total_cost = all.cost.max(0.01); // avoid div by zero

    let proj_header = Line::from(vec![
        Span::styled("   projects", Style::default().fg(ACCENT)),
        Span::styled(
            format!("{}cost        %    sessions    last",
                " ".repeat((w as usize).saturating_sub(58).max(2))),
            Style::default().fg(FG_FAINT),
        ),
    ]);
    frame.render_widget(Paragraph::new(proj_header), chunks[6]);

    // ── Project list ──
    let projects = store.by_project();
    let max_rows = chunks[7].height as usize;
    let proj_scroll = state.project_scroll.min(projects.len().saturating_sub(max_rows));

    let mut project_lines: Vec<Line> = Vec::new();
    for (i, p) in projects.iter().skip(proj_scroll).take(max_rows).enumerate() {
        let abs_idx = proj_scroll + i;
        let is_selected = in_project_zone && abs_idx == state.project_cursor;
        let name_w = (w as usize).saturating_sub(52).max(12);
        let pct = p.cost / total_cost * 100.0;

        let cursor_char = if is_selected { "\u{25b8}" } else { " " };
        let fg = if is_selected { FG } else { FG_MUTED };
        let cost_fg = if is_selected { ACCENT } else { FG_MUTED };

        // Mini allocation bar (5 chars wide)
        let alloc_filled = (pct / 100.0 * 5.0).round() as usize;
        let alloc_bar: String = "\u{2588}".repeat(alloc_filled.min(5));
        let alloc_empty: String = "\u{2591}".repeat(5usize.saturating_sub(alloc_filled));

        project_lines.push(Line::from(vec![
            Span::styled(format!("  {} ", cursor_char), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(
                format!("{:<width$}", truncate(&p.name, name_w), width = name_w),
                Style::default().fg(fg),
            ),
            Span::styled(format!("{:>8}", pricing::format_cost(p.cost)), Style::default().fg(cost_fg)),
            Span::styled("  ", Style::default()),
            Span::styled(alloc_bar, Style::default().fg(ACCENT)),
            Span::styled(alloc_empty, Style::default().fg(FG_FAINT)),
            Span::styled(format!("{:>4.0}%", pct), Style::default().fg(FG_FAINT)),
            Span::styled(format!("   {:>5}", p.session_count), Style::default().fg(FG_FAINT)),
            Span::styled(format!("    {:>8}", format_ago(p.last_used)), Style::default().fg(FG_FAINT)),
        ]));
    }

    if projects.len() > max_rows {
        let remaining = projects.len().saturating_sub(proj_scroll + max_rows);
        if remaining > 0 {
            if let Some(last) = project_lines.last_mut() {
                *last = Line::from(vec![
                    Span::styled(
                        format!("     ... {} more", remaining),
                        Style::default().fg(FG_FAINT),
                    ),
                ]);
            }
        }
    }
    frame.render_widget(Paragraph::new(project_lines), chunks[7]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[8]);

    // ── Help bar ──
    let help = help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("tab", "switch"),
        ("enter", "detail"),
        ("d", "daily"),
        ("t", "trends"),
        ("s", "sessions"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[9]);
}

// ════════════════════════════════════════════════════════════════════════
//  Session health verdict
// ════════════════════════════════════════════════════════════════════════

fn session_health(analysis: &crate::store::SessionAnalysis, ctx_pct: f64) -> (&'static str, Color) {
    // Red: context near limit with poor efficiency
    if ctx_pct > 85.0 {
        return ("START NEW SESSION  context near limit", RED);
    }

    // Red: cost is high and output yield is tanking
    if analysis.context_growth > 6.0 && analysis.output_efficiency < 0.1 {
        return ("CONSIDER NEW SESSION  yield declining, cost rising", RED);
    }

    // Yellow: context growing fast
    if ctx_pct > 70.0 && analysis.context_growth > 4.0 {
        return ("SESSION AGING  context growing fast", YELLOW);
    }

    // Yellow: many turns since last compaction, high growth
    if analysis.messages_since_compaction > 30 && analysis.context_growth > 3.0 {
        return ("SESSION AGING  long since compaction", YELLOW);
    }

    // Green
    if ctx_pct < 40.0 {
        return ("SESSION FRESH", Color::Rgb(120, 190, 120));
    }

    ("SESSION OK", FG_FAINT)
}

// ════════════════════════════════════════════════════════════════════════
//  Budget span (inline in KPI strip)
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
    Span::styled(format!("     {} budget {:.0}%", label, pct), Style::default().fg(color))
}

// ════════════════════════════════════════════════════════════════════════
//  Detail view (unchanged structure, minor polish)
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
            Constraint::Length(3),   // header
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // context timeline
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help
        ])
        .split(area);

    // ── Header ──
    if let Some(meta) = meta {
        let cost = detail.timeline.total_cost;
        let dur = detail.timeline.duration_minutes;
        let dur_str = if dur >= 60 {
            format!("{}h{:02}m", dur / 60, dur % 60)
        } else {
            format!("{}m", dur.max(1))
        };

        let grade_str = if let Some(ref a) = analysis {
            let g = grade_letter(a);
            let gc = match g {
                "A" => Color::Rgb(120, 190, 120),
                "B" => ACCENT,
                "C" => YELLOW,
                _ => RED,
            };
            (g, gc)
        } else {
            ("-", FG_FAINT)
        };

        let ctx_pct = analysis.as_ref().map(|a| {
            (a.context_current as f64 / 167_000.0 * 100.0).min(100.0)
        }).unwrap_or(0.0);

        let health = analysis.as_ref().map(|a| session_health(a, ctx_pct)).unwrap_or(("", FG_FAINT));

        let header = vec![
            Line::from(vec![
                Span::styled(format!("   {}", truncate(&meta.first_message, (w as usize).saturating_sub(10))),
                    Style::default().fg(FG).bold()),
            ]),
            Line::from(vec![
                Span::styled(format!("   {}", meta.project), Style::default().fg(ACCENT)),
                Span::styled(format!("  {}  {}  {}  ", dur_str, pricing::format_cost(cost), meta.user_count),
                    Style::default().fg(FG_MUTED)),
                Span::styled("turns  ", Style::default().fg(FG_FAINT)),
                Span::styled(grade_str.0, Style::default().fg(grade_str.1).bold()),
                if detail.timeline.compaction_count > 0 {
                    Span::styled(
                        format!("  {} compactions", detail.timeline.compaction_count),
                        Style::default().fg(YELLOW),
                    )
                } else {
                    Span::raw("")
                },
                Span::styled(format!("  {}", health.0), Style::default().fg(health.1)),
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Context Timeline ──
    let turns = &detail.timeline.turns;
    let bar_w = (w as usize).saturating_sub(40).max(10);
    let start_time = turns.first().map(|t| t.timestamp).unwrap_or_else(chrono::Utc::now);

    let thresholds = [25.0, 50.0, 75.0, 85.0];
    let mut last_crossed: Option<usize> = None;
    let mut timeline_lines: Vec<Line> = Vec::new();

    for (i, turn) in turns.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == turns.len() - 1;
        let is_compaction = turn.is_compaction;
        let is_expensive = turn.cost > 0.50;

        let current_threshold = thresholds.iter().rposition(|&t| turn.context_pct >= t);
        let crossed_new = current_threshold != last_crossed;
        if crossed_new { last_crossed = current_threshold; }

        let is_notable = is_first || is_last || is_compaction || is_expensive || crossed_new;
        if !is_notable { continue; }

        let elapsed = (turn.timestamp - start_time).num_minutes();
        let time_str = format!("{:>3}m", elapsed);

        let filled = ((turn.context_pct / 100.0) * bar_w as f64).round() as usize;
        let bar_filled: String = "\u{2588}".repeat(filled);
        let bar_empty: String = "\u{2591}".repeat(bar_w.saturating_sub(filled));

        let bar_color = if turn.context_pct > 85.0 { RED }
            else if turn.context_pct > 60.0 { YELLOW }
            else { Color::Rgb(120, 190, 120) };

        let event_label = if is_first {
            "started"
        } else if is_compaction {
            "\u{2193} compacted"
        } else if is_last {
            "current"
        } else if turn.context_pct > 85.0 {
            "\u{26a0} near limit"
        } else if is_expensive {
            "cost spike"
        } else {
            ""
        };

        let event_color = if is_compaction { YELLOW }
            else if turn.context_pct > 85.0 { RED }
            else if is_expensive { YELLOW }
            else if is_last { ACCENT }
            else { FG_FAINT };

        timeline_lines.push(Line::from(vec![
            Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
            Span::styled(bar_filled, Style::default().fg(bar_color)),
            Span::styled(bar_empty, Style::default().fg(FG_FAINT)),
            Span::styled(
                format!(" {:>3.0}%", turn.context_pct),
                Style::default().fg(FG_MUTED),
            ),
            Span::styled(
                format!("  {}", compact(turn.context_size)),
                Style::default().fg(FG_FAINT),
            ),
            if !event_label.is_empty() {
                Span::styled(format!("  {}", event_label), Style::default().fg(event_color))
            } else {
                Span::raw("")
            },
        ]));
    }

    let total_turns = turns.len();
    let shown = timeline_lines.len();
    if total_turns > shown {
        timeline_lines.push(Line::from(vec![
            Span::styled(
                format!("   {} turns total, {} shown", total_turns, shown),
                Style::default().fg(FG_FAINT),
            ),
        ]));
    }

    // Cost per turn sparkline
    let costs: Vec<f64> = turns.iter().map(|t| t.cost).collect();
    if !costs.is_empty() {
        let (peak_idx, peak_cost) = costs.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, &c)| (i, c))
            .unwrap_or((0, 0.0));
        let cost_spark = spark(&costs);

        timeline_lines.push(Line::from(Span::raw("")));
        timeline_lines.push(Line::from(vec![
            Span::styled("   cost/turn ", Style::default().fg(FG_FAINT)),
            Span::styled(cost_spark, Style::default().fg(ACCENT)),
            Span::styled(
                format!("  peak {} at turn {}", pricing::format_cost(peak_cost), peak_idx + 1),
                Style::default().fg(FG_MUTED),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(timeline_lines), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    let help = help_bar(&[("esc", "back"), ("q", "quit")]);
    frame.render_widget(Paragraph::new(help), chunks[4]);
}

// ════════════════════════════════════════════════════════════════════════
//  Helpers
// ════════════════════════════════════════════════════════════════════════

fn grade_letter(analysis: &crate::store::SessionAnalysis) -> &'static str {
    let mut score = 100i32;
    if analysis.context_growth > 8.0 { score -= 30; }
    else if analysis.context_growth > 5.0 { score -= 20; }
    else if analysis.context_growth > 3.0 { score -= 10; }
    if analysis.output_efficiency < 0.1 { score -= 30; }
    else if analysis.output_efficiency < 0.2 { score -= 15; }
    if analysis.cost_per_1k_output > 1.0 { score -= 20; }
    else if analysis.cost_per_1k_output > 0.5 { score -= 10; }
    if analysis.compaction_count > 0 { score += 5; }
    match score {
        90..=200 => "A",
        75..=89 => "B",
        60..=74 => "C",
        40..=59 => "D",
        _ => "F",
    }
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > max {
        format!("{}...", &first_line[..max.saturating_sub(3)])
    } else {
        first_line.to_string()
    }
}

