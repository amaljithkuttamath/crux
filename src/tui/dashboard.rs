use crate::config::Config;
use crate::pricing;
use crate::store::{Store, SessionTimeline};
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;

#[derive(PartialEq, Clone, Copy)]
pub enum FocusZone {
    ActiveSessions,
    Projects,
}

pub struct DashboardState {
    pub focus: FocusZone,
    pub active_cursor: usize,
    pub project_cursor: usize,
    pub project_scroll: usize,
    pub detail: Option<SessionDetailView>,
}

pub struct SessionDetailView {
    pub session_id: String,
    pub timeline: SessionTimeline,
    pub scroll: usize,
}

impl DashboardState {
    pub fn new() -> Self {
        Self {
            focus: FocusZone::ActiveSessions,
            active_cursor: 0,
            project_cursor: 0,
            project_scroll: 0,
            detail: None,
        }
    }

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
                    // Keep cursor visible (assume ~10 rows visible, will be adjusted at render)
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

        let active = store.active_sessions(24);
        if let Some((meta, _)) = active.get(self.active_cursor) {
            if let Some(timeline) = store.session_timeline(&meta.session_id) {
                self.detail = Some(SessionDetailView {
                    session_id: meta.session_id.clone(),
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

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &DashboardState) {
    if state.detail.is_some() {
        render_detail(frame, store, config, state);
    } else {
        render_main(frame, store, config, state);
    }
}

fn render_main(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &DashboardState) {
    let area = frame.area();
    let w = area.width;

    let active = store.active_sessions(24);
    let active_count = active.len();

    let active_height = if active_count > 0 {
        (active_count as u16 * 3 + 2).min(12)
    } else {
        2
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),              // title
            Constraint::Length(active_height),   // active sessions
            Constraint::Length(1),              // divider
            Constraint::Length(6),              // today + cost rate
            Constraint::Length(1),              // divider
            Constraint::Length(1),              // projects header
            Constraint::Min(3),                // project list
            Constraint::Length(1),              // divider
            Constraint::Length(1),              // help bar
        ])
        .split(area);

    // ── Title ──
    let today_agg = store.today();
    let week_agg = store.this_week();
    let streak = store.streak_days();

    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("   usagetracker", Style::default().fg(ACCENT).bold()),
            Span::styled(
                format!("{}sessions: {} today / {} this week   streak: {}d",
                    " ".repeat((w as usize).saturating_sub(65)),
                    today_agg.session_count,
                    week_agg.session_count,
                    streak,
                ),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ]);
    frame.render_widget(title, chunks[0]);

    // ── Active Sessions (hero section) ──
    let in_active_zone = state.focus == FocusZone::ActiveSessions;

    if active_count > 0 {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("   LIVE", Style::default().fg(Color::Rgb(120, 190, 120)).bold()),
                Span::styled(
                    format!("  {} active session{}", active_count, if active_count > 1 { "s" } else { "" }),
                    Style::default().fg(FG_MUTED),
                ),
            ]),
        ];

        for (i, (meta, analysis)) in active.iter().take(3).enumerate() {
            let is_selected = in_active_zone && i == state.active_cursor;
            let cost = analysis.total_cost;
            let grade = grade_letter(analysis);
            let grade_color = match grade {
                "A" => Color::Rgb(120, 190, 120),
                "B" => ACCENT,
                "C" => YELLOW,
                _ => RED,
            };

            let ctx_pct = (analysis.context_current as f64 / 167_000.0 * 100.0).min(100.0);
            let bar_w = 20usize;
            let bar_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { Color::Rgb(120, 190, 120) };

            let name_w = (w as usize).saturating_sub(78).max(8);
            let topic = truncate(&meta.first_message, name_w);

            let cursor_char = if is_selected { ">" } else { " " };
            let fg = if is_selected { FG } else { FG_MUTED };

            // Line 1: cursor + topic + metrics
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", cursor_char), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
                Span::styled(format!("{:<width$}", topic, width = name_w), Style::default().fg(fg)),
                Span::styled(format!("  {}m", meta.user_count), Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {}", pricing::format_cost(cost)), Style::default().fg(if is_selected { ACCENT } else { FG_MUTED })),
                Span::styled(format!("  {}", grade), Style::default().fg(grade_color).bold()),
                Span::styled(format!("  {}", format_ago_short(meta.end_time)), Style::default().fg(FG_FAINT)),
            ]));

            // Line 2: context bar + agents
            let (bar_f, bar_e) = smooth_bar(ctx_pct, 100.0, bar_w);
            let mut ctx_spans = vec![
                Span::styled("     ctx ", Style::default().fg(FG_FAINT)),
                Span::styled(bar_f, Style::default().fg(bar_color)),
                Span::styled(bar_e, Style::default().fg(FG_FAINT)),
                Span::styled(
                    format!(" {:.0}%  {}  {:.1}x",
                        ctx_pct, compact(analysis.context_current), analysis.context_growth),
                    Style::default().fg(FG_MUTED),
                ),
                Span::styled(
                    format!("  cache {:.0}%", analysis.cache_hit_rate * 100.0),
                    Style::default().fg(FG_FAINT),
                ),
            ];
            if analysis.compaction_count > 0 {
                ctx_spans.push(Span::styled(
                    format!("  {} compactions", analysis.compaction_count),
                    Style::default().fg(FG_FAINT),
                ));
            }
            if meta.agent_spawns > 0 {
                ctx_spans.push(Span::styled(
                    format!("  {} agents", meta.agent_spawns),
                    Style::default().fg(YELLOW),
                ));
            }
            lines.push(Line::from(ctx_spans));

            if active.len() > 1 {
                lines.push(Line::from(Span::raw("")));
            }
        }

        frame.render_widget(Paragraph::new(lines), chunks[1]);
    } else {
        let no_active = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("   no active sessions", Style::default().fg(FG_FAINT)),
            ]),
        ]);
        frame.render_widget(no_active, chunks[1]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── Summary: 2-column layout ──
    let mode = LayoutMode::from_width(w);
    let today = store.today();
    let yesterday = store.yesterday();
    let week = store.this_week();

    match mode {
        LayoutMode::Compact => {
            let period = vec![
                period_line("   today      ", &today, true, String::new(), FG_FAINT),
                period_line("   yesterday  ", &yesterday, false, String::new(), FG_FAINT),
                period_line("   this week  ", &week, false, String::new(), FG_FAINT),
            ];
            frame.render_widget(Paragraph::new(period), chunks[3]);
        }
        _ => {
            let summary_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(55),
                    Constraint::Percentage(45),
                ])
                .split(chunks[3]);

            let (budget_str, budget_pct) = if let Some(budget) = config.budget_daily {
                let p = today.cost / budget * 100.0;
                (format!("  {:.0}% of daily budget", p), Some(p))
            } else if let Some(budget) = config.budget_weekly {
                let p = week.cost / budget * 100.0;
                (format!("  {:.0}% of weekly budget", p), Some(p))
            } else {
                (String::new(), None)
            };

            let budget_color = match budget_pct {
                Some(p) if p > 90.0 => RED,
                Some(p) if p > 70.0 => YELLOW,
                _ => FG_FAINT,
            };

            let left = vec![
                period_line("   today      ", &today, true, String::new(), FG_FAINT),
                period_line("   yesterday  ", &yesterday, false, String::new(), FG_FAINT),
                period_line("   this week  ", &week, false, String::new(), FG_FAINT),
                if budget_pct.is_some() {
                    let bp = budget_pct.unwrap_or(0.0);
                    let bw = 15usize;
                    let (bf, be) = smooth_bar(bp, 100.0, bw);
                    Line::from(vec![
                        Span::styled("   budget ", Style::default().fg(FG_MUTED)),
                        Span::styled(bf, Style::default().fg(budget_color)),
                        Span::styled(be, Style::default().fg(FG_FAINT)),
                        Span::styled(format!(" {:.0}%", bp), Style::default().fg(budget_color)),
                        Span::styled(budget_str, Style::default().fg(FG_FAINT)),
                    ])
                } else {
                    Line::from(Span::raw(""))
                },
            ];
            frame.render_widget(Paragraph::new(left), summary_cols[0]);

            let days_data = store.by_day(7);
            let sessions_spark = store.sessions_per_day(7);
            let spark_str = spark(&sessions_spark);

            let week_sessions: usize = days_data.iter().map(|d| d.session_count).sum();
            let mut right_lines = vec![
                Line::from(vec![
                    Span::styled("  7d ", Style::default().fg(FG_FAINT)),
                    Span::styled(spark_str, Style::default().fg(ACCENT)),
                    Span::styled(format!("  {} sessions", week_sessions), Style::default().fg(FG_MUTED)),
                ]),
            ];

            let max_tokens = days_data.iter()
                .map(|d| d.input_tokens + d.output_tokens + d.cache_creation_tokens + d.cache_read_tokens)
                .max()
                .unwrap_or(1);
            let mini_bar_w = 10usize;

            for day in days_data.iter().take(5).rev() {
                let total = day.input_tokens + day.output_tokens + day.cache_creation_tokens + day.cache_read_tokens;
                let weekday = day.date.format("%a").to_string();
                let (bf, be) = smooth_bar(total as f64, max_tokens as f64, mini_bar_w);
                right_lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", weekday), Style::default().fg(FG_FAINT)),
                    Span::styled(bf, Style::default().fg(ACCENT)),
                    Span::styled(be, Style::default().fg(FG_FAINT)),
                    Span::styled(format!(" {}  {}s", compact(total), day.session_count), Style::default().fg(FG_MUTED)),
                ]));
            }

            frame.render_widget(Paragraph::new(right_lines), summary_cols[1]);
        }
    }
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // ── Projects header ──
    let in_project_zone = state.focus == FocusZone::Projects;
    let proj_header = Line::from(vec![
        Span::styled("   projects", Style::default().fg(ACCENT)),
        Span::styled(
            format!("{}tokens       cost   sessions    last",
                " ".repeat((w as usize).saturating_sub(62).max(2))),
            Style::default().fg(FG_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(proj_header), chunks[5]);

    // ── Project list ──
    let projects = store.by_project();
    let max_rows = chunks[6].height as usize;
    let proj_scroll = state.project_scroll.min(projects.len().saturating_sub(max_rows));

    let mut project_lines: Vec<Line> = Vec::new();
    for (i, p) in projects.iter().skip(proj_scroll).take(max_rows).enumerate() {
        let abs_idx = proj_scroll + i;
        let is_selected = in_project_zone && abs_idx == state.project_cursor;
        let total = p.input_tokens + p.output_tokens + p.cache_creation_tokens + p.cache_read_tokens;
        let name_w = (w as usize).saturating_sub(58).max(12);

        let cursor_char = if is_selected { ">" } else { " " };
        let fg = if is_selected { FG } else { FG_MUTED };
        let cost_fg = if is_selected { ACCENT } else { FG_MUTED };

        project_lines.push(Line::from(vec![
            Span::styled(format!("  {} ", cursor_char), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(
                format!("{:<width$}", truncate(&p.name, name_w), width = name_w),
                Style::default().fg(fg),
            ),
            Span::styled(format!("{:>10}", compact(total)), Style::default().fg(fg)),
            Span::styled(format!("  {:>8}", pricing::format_cost(p.cost)), Style::default().fg(cost_fg)),
            Span::styled(format!("   {:>5}", p.session_count), Style::default().fg(FG_MUTED)),
            Span::styled(format!("    {:>8}", format_ago(p.last_used)), Style::default().fg(FG_MUTED)),
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
    frame.render_widget(Paragraph::new(project_lines), chunks[6]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[7]);

    // ── Help bar ──
    let help = Line::from(vec![
        Span::styled("   \u{2191}\u{2193}", Style::default().fg(ACCENT)),
        Span::styled(" navigate  ", Style::default().fg(FG_MUTED)),
        Span::styled("tab", Style::default().fg(ACCENT)),
        Span::styled(" switch  ", Style::default().fg(FG_MUTED)),
        Span::styled("enter", Style::default().fg(ACCENT)),
        Span::styled(" detail  ", Style::default().fg(FG_MUTED)),
        Span::styled("d", Style::default().fg(ACCENT)),
        Span::styled(" daily  ", Style::default().fg(FG_MUTED)),
        Span::styled("t", Style::default().fg(ACCENT)),
        Span::styled(" trends  ", Style::default().fg(FG_MUTED)),
        Span::styled("s", Style::default().fg(ACCENT)),
        Span::styled(" sessions  ", Style::default().fg(FG_MUTED)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[8]);
}

fn render_detail(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &DashboardState) {
    let area = frame.area();
    let w = area.width;
    let detail = match &state.detail {
        Some(d) => d,
        None => return,
    };

    // Find session meta
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

        let header = vec![
            Line::from(vec![
                Span::styled(format!("   {}", truncate(&meta.first_message, (w as usize).saturating_sub(10))),
                    Style::default().fg(FG).bold()),
            ]),
            Line::from(vec![
                Span::styled(format!("   {}", meta.project), Style::default().fg(ACCENT)),
                Span::styled(format!("  {}  {}  {}  ", dur_str, pricing::format_cost(cost), meta.user_count),
                    Style::default().fg(FG_MUTED)),
                Span::styled(format!("turns  Grade "), Style::default().fg(FG_FAINT)),
                Span::styled(grade_str.0, Style::default().fg(grade_str.1).bold()),
                if detail.timeline.compaction_count > 0 {
                    Span::styled(
                        format!("  {} compactions", detail.timeline.compaction_count),
                        Style::default().fg(FG_FAINT),
                    )
                } else {
                    Span::raw("")
                },
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Context Timeline (notable events only) ──
    let turns = &detail.timeline.turns;
    let bar_w = (w as usize).saturating_sub(40).max(10);
    let start_time = turns.first().map(|t| t.timestamp).unwrap_or_else(chrono::Utc::now);

    // Only show turns where something notable happened:
    // first, last, threshold crossings (25/50/75/85%), compactions, expensive turns
    let thresholds = [25.0, 50.0, 75.0, 85.0];
    let mut last_crossed: Option<usize> = None; // index of last threshold crossed
    let mut timeline_lines: Vec<Line> = Vec::new();

    for (i, turn) in turns.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == turns.len() - 1;
        let is_compaction = turn.is_compaction;
        let is_expensive = turn.cost > 0.50;

        // Check if we crossed a new threshold
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
            "" // threshold crossing, bar speaks for itself
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

    // Summary line at bottom
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

    frame.render_widget(Paragraph::new(timeline_lines), chunks[2]);

    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // ── Help ──
    let help = Line::from(vec![
        Span::styled("   esc", Style::default().fg(ACCENT)),
        Span::styled(" back   ", Style::default().fg(FG_MUTED)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[4]);
}

fn period_line<'a>(label: &'a str, agg: &crate::store::Aggregation, highlight: bool, extra: String, extra_color: Color) -> Line<'a> {
    let fg = if highlight { FG } else { FG_MUTED };
    let cost_fg = if highlight { ACCENT } else { FG_FAINT };

    let mut spans = vec![
        Span::styled(label.to_string(), Style::default().fg(fg)),
        Span::styled(format!("{:>10}", compact(agg.total_tokens())), Style::default().fg(fg)),
        Span::styled(format!("  {}", pricing::format_cost(agg.cost)), Style::default().fg(cost_fg)),
        Span::styled(format!("   {} sess", agg.session_count), Style::default().fg(FG_FAINT)),
    ];
    if !extra.is_empty() {
        spans.push(Span::styled(extra, Style::default().fg(extra_color)));
    }
    Line::from(spans)
}

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

fn format_ago_short(time: chrono::DateTime<chrono::Utc>) -> String {
    let diff = chrono::Utc::now() - time;
    if diff.num_minutes() < 2 {
        "now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else {
        format!("{}d ago", diff.num_days())
    }
}
