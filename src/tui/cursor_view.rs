use crate::config::Config;
use crate::parser::conversation::{SessionStatus, SessionMeta, SessionMode};
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use chrono::{Datelike, Utc};
use ratatui::prelude::*;
use ratatui::widgets::*;

#[derive(Default)]
pub struct CursorViewState {
    pub cursor: usize,
    pub scroll: usize,
    pub detail: Option<CursorSessionDetail>,
    pub detail_scroll: usize,
}

pub struct CursorSessionDetail {
    pub session_id: String,
}

impl CursorViewState {
    pub fn move_up(&mut self) {
        if self.detail.is_some() {
            self.detail_scroll = self.detail_scroll.saturating_sub(1);
        } else if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self, max: usize) {
        if self.detail.is_some() {
            self.detail_scroll += 1;
        } else if self.cursor + 1 < max {
            self.cursor += 1;
        }
    }

    pub fn enter(&mut self, store: &Store) {
        if self.detail.is_some() { return; }
        let sessions = store.cursor_sessions();
        if let Some(session) = sessions.get(self.cursor) {
            self.detail = Some(CursorSessionDetail {
                session_id: session.session_id.clone(),
            });
            self.detail_scroll = 0;
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

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut CursorViewState) {
    if state.detail.is_some() {
        render_detail(frame, store, config, state);
    } else {
        render_main(frame, store, config, state);
    }
}

// ════════════════════════════════════════════════════════════════════
//  Main Cursor view: analytics top + session list
// ════════════════════════════════════════════════════════════════════

fn render_main(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &mut CursorViewState) {
    let area = frame.area();
    let w = area.width;
    let sessions = store.cursor_sessions();
    let stats = store.cursor_overview_stats();
    let model_stats = store.cursor_model_stats();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),   // title
            Constraint::Length(1),   // divider
            Constraint::Length(3),   // KPIs + model comparison bar chart
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // session list
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help
        ])
        .split(area);

    // ── Title ──
    let trend_str = if stats.cost_trend_pct > 0 {
        format!("+{}%", stats.cost_trend_pct)
    } else if stats.cost_trend_pct < 0 {
        format!("{}%", stats.cost_trend_pct)
    } else {
        "flat".to_string()
    };
    let title = Line::from(vec![
        Span::styled("   ", Style::default()),
        Span::styled("\u{25cf}", Style::default().fg(BLUE)),
        Span::styled(" Cursor", Style::default().fg(FG).bold()),
        Span::styled(
            format!("{}{}  {}  {} sessions",
                " ".repeat((w as usize).saturating_sub(50).max(1)),
                pricing::format_cost(stats.total_cost),
                trend_str,
                stats.total_sessions,
            ),
            Style::default().fg(FG_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Analytics zone: KPIs + model comparison ──
    let completion_c = if stats.completion_rate > 70.0 { GREEN } else if stats.completion_rate > 50.0 { YELLOW } else { RED };
    let ctx_c = if stats.avg_context_fill < 50.0 { GREEN } else if stats.avg_context_fill < 75.0 { YELLOW } else { RED };

    let kpi_line1 = Line::from(vec![
        Span::styled("   completion ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.0}%", stats.completion_rate), Style::default().fg(completion_c).bold()),
        Span::styled("   lines shipped ", Style::default().fg(FG_FAINT)),
        Span::styled(compact(stats.total_lines), Style::default().fg(FG).bold()),
        Span::styled("   avg ctx ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.0}%", stats.avg_context_fill), Style::default().fg(ctx_c).bold()),
        Span::styled("   agent mode ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.0}%", stats.agent_pct), Style::default().fg(PURPLE).bold()),
    ]);

    // Model comparison: horizontal bars showing completion rate per model
    let mut model_line_spans: Vec<Span> = vec![Span::styled("   ", Style::default())];
    for ms in model_stats.iter().take(4) {
        let name = truncate_model(&ms.model, 10);
        let c = if ms.completion_rate > 70.0 { GREEN } else if ms.completion_rate > 40.0 { YELLOW } else { RED };
        let bar_w = 6;
        let filled = ((ms.completion_rate / 100.0) * bar_w as f64).round() as usize;
        let bar_f: String = "\u{2588}".repeat(filled.min(bar_w));
        let bar_e: String = "\u{2591}".repeat(bar_w.saturating_sub(filled));
        model_line_spans.push(Span::styled(format!("{} ", name), Style::default().fg(FG_FAINT)));
        model_line_spans.push(Span::styled(bar_f, Style::default().fg(c)));
        model_line_spans.push(Span::styled(bar_e, Style::default().fg(FG_FAINT)));
        model_line_spans.push(Span::styled(format!(" {:.0}%  ", ms.completion_rate), Style::default().fg(FG_MUTED)));
    }
    let model_line = Line::from(model_line_spans);

    // Sparkline for session volume
    let spark_str = spark(&stats.monthly_volumes);
    let spark_line = Line::from(vec![
        Span::styled("   7mo ", Style::default().fg(FG_FAINT)),
        Span::styled(spark_str, Style::default().fg(BLUE)),
        Span::styled(format!("  {:.0} lines/sess  {} files shipped",
            stats.avg_lines_per_session, compact(stats.total_files)),
            Style::default().fg(FG_FAINT)),
    ]);

    frame.render_widget(Paragraph::new(vec![kpi_line1, model_line, spark_line]), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // ── Session list with date grouping ──
    let max_rows = chunks[4].height as usize;
    let mut lines: Vec<Line> = Vec::new();
    let mut current_date_label = String::new();
    let now = Utc::now();

    if state.cursor >= state.scroll + max_rows {
        state.scroll = state.cursor.saturating_sub(max_rows - 1);
    }
    if state.cursor < state.scroll {
        state.scroll = state.cursor;
    }

    let mut row_idx = 0usize;
    let mut visible_row = 0usize;

    for (i, session) in sessions.iter().enumerate() {
        let date = session.start_time.date_naive();
        let label = if date == now.date_naive() {
            "TODAY".to_string()
        } else if date == (now - chrono::Duration::days(1)).date_naive() {
            "YESTERDAY".to_string()
        } else {
            format!("{} {}", month_abbrev(date.month()), date.day())
        };

        if label != current_date_label {
            if row_idx >= state.scroll && visible_row < max_rows {
                if !current_date_label.is_empty() {
                    lines.push(Line::from(Span::raw("")));
                    visible_row += 1;
                    if visible_row >= max_rows { break; }
                }
                lines.push(Line::from(vec![
                    Span::styled(format!("   {}", label), Style::default().fg(FG_MUTED).bold()),
                ]));
                visible_row += 1;
            }
            current_date_label = label;
            row_idx += 1;
            if visible_row >= max_rows { break; }
        }

        if row_idx < state.scroll { row_idx += 1; continue; }
        if visible_row >= max_rows { break; }

        let is_selected = i == state.cursor;
        let dur = session.duration_minutes();
        let dur_str = if dur >= 60 { format!("{}h{:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };

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

        let model = store.session_model(&session.session_id);
        let model_short = truncate_model(&model, 14);
        let mode_badge = match session.cursor_mode {
            Some(SessionMode::Agent) => "agt",
            Some(SessionMode::Chat) => "chat",
            _ => "",
        };

        let ctx_str = session.context_usage_pct.map(|p| format!("{:.0}%", p)).unwrap_or_default();
        let ctx_color = session.context_usage_pct
            .map(|p| if p > 85.0 { RED } else if p > 60.0 { YELLOW } else { FG_FAINT })
            .unwrap_or(FG_FAINT);

        let lines_str = format_lines(session);

        let fg = if is_selected { FG } else { FG_MUTED };
        let cursor_char = if is_selected { "\u{25b8}" } else { " " };

        let time_str = session.start_time.format("%H:%M").to_string();
        let name_w = (w as usize).saturating_sub(75).max(8);
        let topic = truncate(&session.first_message, name_w);

        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", cursor_char), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(status_icon, Style::default().fg(status_color)),
            Span::styled(format!(" {} ", time_str), Style::default().fg(FG_FAINT)),
            Span::styled(format!("{:<width$}", topic, width = name_w), Style::default().fg(fg)),
            Span::styled(format!("  {:<14}", model_short), Style::default().fg(FG_FAINT)),
            Span::styled(format!("{:<4}", mode_badge), Style::default().fg(if mode_badge == "agt" { PURPLE } else { FG_FAINT })),
            Span::styled(format!(" {:>4}", dur_str), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:<5}", status_text), Style::default().fg(status_color)),
            Span::styled(format!("{:>4}", ctx_str), Style::default().fg(ctx_color)),
            Span::styled(format!("  {:>8}", lines_str), Style::default().fg(FG_MUTED)),
        ]));
        visible_row += 1;
        row_idx += 1;
    }

    if sessions.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("   no cursor sessions found", Style::default().fg(FG_FAINT)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), chunks[4]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[5]);

    let help = help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("enter", "detail"),
        ("esc", "back"),
        ("d", "claude code"),
        ("h", "history"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[6]);
}

// ════════════════════════════════════════════════════════════════════
//  Session detail (drill-down)
// ════════════════════════════════════════════════════════════════════

fn render_detail(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &mut CursorViewState) {
    let area = frame.area();
    let w = area.width;
    let detail = match &state.detail {
        Some(d) => d,
        None => return,
    };

    let sessions = store.cursor_sessions();
    let meta = match sessions.iter().find(|s| s.session_id == detail.session_id) {
        Some(m) => *m,
        None => return,
    };

    let cost = store.session_cost(&detail.session_id);
    let model = store.session_model(&detail.session_id);
    let has_todos = meta.cursor_todos.as_ref().map(|t| !t.is_empty()).unwrap_or(false);
    let todo_height = if has_todos {
        (meta.cursor_todos.as_ref().map(|t| t.len()).unwrap_or(0) as u16 + 1).min(8)
    } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(if has_todos { 1 } else { 0 }),
            Constraint::Length(todo_height),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // ── Header ──
    let dur = meta.duration_minutes();
    let dur_str = if dur >= 60 { format!("{}h{:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };
    let (status_text, status_color) = match meta.cursor_status {
        Some(SessionStatus::Completed) => ("completed", GREEN),
        Some(SessionStatus::Aborted) => ("aborted", RED),
        _ => ("", FG_FAINT),
    };

    let header = vec![
        Line::from(vec![
            Span::styled(format!("   {}", truncate(&meta.first_message, (w as usize).saturating_sub(10))),
                Style::default().fg(FG).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!("   {}", status_text), Style::default().fg(status_color).bold()),
            Span::styled(format!("  {}", dur_str), Style::default().fg(FG_MUTED)),
        ]),
        Line::from(vec![
            Span::styled(format!("   {}", truncate_model(&model, 30)), Style::default().fg(BLUE)),
            Span::styled(
                format!("  context: {}",
                    meta.context_usage_pct.map(|p| format!("{:.0}%", p)).unwrap_or("n/a".to_string()),
                ),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(header), chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Context trajectory ──
    let mut ctx_lines: Vec<Line> = Vec::new();
    if let (Some(used), Some(limit)) = (meta.context_tokens_used, meta.context_token_limit) {
        let pct = meta.context_usage_pct.unwrap_or(0.0);
        let bar_w = (w as usize).saturating_sub(30).max(10);
        let (bf, be) = smooth_bar(pct, 100.0, bar_w);
        let bar_color = if pct > 85.0 { RED } else if pct > 60.0 { YELLOW } else { GREEN };

        ctx_lines.push(Line::from(vec![
            Span::styled("   ctx  ", Style::default().fg(FG_FAINT)),
            Span::styled(bf, Style::default().fg(bar_color)),
            Span::styled(be, Style::default().fg(FG_FAINT)),
            Span::styled(format!(" {:.0}%", pct), Style::default().fg(bar_color).bold()),
        ]));
        ctx_lines.push(Line::from(vec![
            Span::styled(format!("   {} / {} tokens", compact(used), compact(limit)), Style::default().fg(FG_FAINT)),
        ]));
    } else {
        ctx_lines.push(Line::from(vec![
            Span::styled("   no context data", Style::default().fg(FG_FAINT)),
        ]));
    }
    while ctx_lines.len() < 3 { ctx_lines.push(Line::from(Span::raw(""))); }
    frame.render_widget(Paragraph::new(ctx_lines), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // ── Stats ──
    let (input_tokens, output_tokens) = store.session_tokens(&detail.session_id);
    let la = meta.lines_added.unwrap_or(0);
    let lr = meta.lines_removed.unwrap_or(0);
    let net = la as i64 - lr as i64;
    let net_str = if net >= 0 { format!("+{}", net) } else { format!("{}", net) };
    let net_color = if net > 0 { GREEN } else if net < 0 { RED } else { FG_FAINT };

    let stat_lines = vec![
        Line::from(vec![
            Span::styled("   cost ", Style::default().fg(FG_FAINT)),
            Span::styled(pricing::format_cost(cost), Style::default().fg(ACCENT).bold()),
            Span::styled(format!("  (in {} + out {})", compact(input_tokens), compact(output_tokens)), Style::default().fg(FG_FAINT)),
        ]),
        Line::from(vec![
            Span::styled("   lines ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("+{}", la), Style::default().fg(GREEN)),
            Span::styled(format!("  -{}", lr), Style::default().fg(RED)),
            Span::styled(format!("  net {}", net_str), Style::default().fg(net_color).bold()),
        ]),
        Line::from(vec![
            Span::styled("   files ", Style::default().fg(FG_FAINT)),
            Span::styled(format!("{} changed", meta.files_changed.unwrap_or(0)), Style::default().fg(FG_MUTED)),
            if meta.is_agentic == Some(true) {
                Span::styled("  agentic", Style::default().fg(PURPLE))
            } else { Span::raw("") },
            if let Some(n) = meta.subagent_count {
                if n > 0 { Span::styled(format!("  {} subagents", n), Style::default().fg(FG_FAINT)) }
                else { Span::raw("") }
            } else { Span::raw("") },
        ]),
    ];
    frame.render_widget(Paragraph::new(stat_lines), chunks[4]);

    // ── Todos ──
    if has_todos {
        frame.render_widget(Paragraph::new(divider(w)), chunks[5]);
        let mut todo_lines: Vec<Line> = vec![
            Line::from(vec![Span::styled("   TASKS", Style::default().fg(FG_MUTED).bold())]),
        ];
        if let Some(todos) = &meta.cursor_todos {
            for todo in todos.iter().take(7) {
                let checkbox = if todo.completed { "[x]" } else { "[ ]" };
                let fg = if todo.completed { FG_FAINT } else { FG_MUTED };
                todo_lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", checkbox), Style::default().fg(if todo.completed { GREEN } else { FG_FAINT })),
                    Span::styled(truncate(&todo.content, (w as usize).saturating_sub(12)), Style::default().fg(fg)),
                ]));
            }
        }
        frame.render_widget(Paragraph::new(todo_lines), chunks[6]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[8]);
    let help = help_bar(&[("esc", "back"), ("\u{2191}\u{2193}", "scroll"), ("q", "quit")]);
    frame.render_widget(Paragraph::new(help), chunks[9]);
}

fn format_lines(session: &SessionMeta) -> String {
    match (session.lines_added, session.lines_removed) {
        (Some(a), Some(r)) => format!("+{} -{}", a, r),
        (Some(a), None) => format!("+{}", a),
        _ => String::new(),
    }
}
