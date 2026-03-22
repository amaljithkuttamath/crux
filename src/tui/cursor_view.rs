use crate::config::Config;
use crate::parser::conversation::{SessionStatus, SessionMeta, SessionMode};
use crate::pricing;
use crate::store::Store;
use crate::store::analysis;
use super::widgets::*;
use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::*;

#[derive(Default, Clone, Copy, PartialEq)]
pub enum SortColumn {
    #[default]
    Time,
    Cost,
    Duration,
    Context,
    Status,
}

impl SortColumn {
    pub fn next(self) -> Self {
        match self {
            Self::Time => Self::Cost,
            Self::Cost => Self::Duration,
            Self::Duration => Self::Context,
            Self::Context => Self::Status,
            Self::Status => Self::Time,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Cost => "cost",
            Self::Duration => "dur",
            Self::Context => "ctx",
            Self::Status => "status",
        }
    }
}

#[derive(Default)]
pub struct CursorViewState {
    pub cursor: usize,
    pub scroll: usize,
    pub detail: Option<CursorSessionDetail>,
    pub detail_scroll: usize,
    pub sort_column: SortColumn,
    pub search_active: bool,
    pub search_query: String,
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

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut CursorViewState, live_sessions: &std::collections::HashMap<String, bool>) {
    if state.detail.is_some() {
        render_detail(frame, store, config, state);
    } else {
        render_main(frame, store, config, state, live_sessions);
    }
}

// ════════════════════════════════════════════════════════════════════
//  Main Cursor view: analytics top + session list
// ════════════════════════════════════════════════════════════════════

fn render_main(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut CursorViewState, live_sessions: &std::collections::HashMap<String, bool>) {
    let area = frame.area();
    let w = area.width;
    let all_sessions = store.cursor_sessions();
    let stats = store.cursor_overview_stats();
    let model_stats = store.cursor_model_stats();
    let today = store.today_by_source(crate::parser::Source::Cursor);
    let now = Utc::now();

    // Count active (live) cursor sessions
    let active_count = all_sessions.iter()
        .filter(|s| live_sessions.get(&s.session_id).copied().unwrap_or(false))
        .count();

    // Filter by search query
    let filtered: Vec<&SessionMeta> = if state.search_query.is_empty() {
        all_sessions.to_vec()
    } else {
        let q = state.search_query.to_lowercase();
        all_sessions.iter().filter(|s| {
            s.first_message.to_lowercase().contains(&q)
                || store.session_model(&s.session_id).to_lowercase().contains(&q)
                || s.project.to_lowercase().contains(&q)
        }).copied().collect()
    };

    // Sort sessions
    let mut sorted: Vec<&SessionMeta> = filtered;
    match state.sort_column {
        SortColumn::Time => {} // already sorted by time (newest first from store)
        SortColumn::Cost => {
            sorted.sort_by(|a, b| {
                let ca = store.session_cost(&a.session_id);
                let cb = store.session_cost(&b.session_id);
                cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SortColumn::Duration => {
            sorted.sort_by_key(|s| std::cmp::Reverse(s.duration_minutes()));
        }
        SortColumn::Context => {
            sorted.sort_by(|a, b| {
                let pa = a.context_usage_pct.unwrap_or(0.0);
                let pb = b.context_usage_pct.unwrap_or(0.0);
                pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SortColumn::Status => {
            sorted.sort_by(|a, b| {
                let sa = cursor_status_order(a, live_sessions);
                let sb = cursor_status_order(b, live_sessions);
                sb.cmp(&sa)
            });
        }
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(2),   // nav header
            Constraint::Length(1),   // source header
            Constraint::Length(1),   // divider
            Constraint::Length(3),   // analytics zone (KPIs + model comparison)
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // column headers
            Constraint::Min(4),     // session list
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help / search
        ])
        .split(area);

    // ── Nav header ──
    let nav = nav_header("cursor", w);
    frame.render_widget(Paragraph::new(nav), chunks[0]);

    // ── Source header ──
    let active_str = if active_count > 0 {
        format!("  {} active", active_count)
    } else {
        String::new()
    };
    let source_header = Line::from(vec![
        Span::styled("   ", Style::default()),
        Span::styled("\u{25cf}", Style::default().fg(BLUE)),
        Span::styled(" Cursor", Style::default().fg(FG).bold()),
        Span::styled(
            format!("{}{}  {} sessions{}",
                " ".repeat((w as usize).saturating_sub(55).max(1)),
                pricing::format_cost(today.cost),
                stats.total_sessions,
                active_str,
            ),
            Style::default().fg(FG_MUTED),
        ),
        Span::styled(
            format!("     sort: {}\u{25bc}", state.sort_column.label()),
            Style::default().fg(FG_FAINT),
        ),
    ]);
    frame.render_widget(Paragraph::new(source_header), chunks[1]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── Analytics zone: KPIs + model comparison (PRESERVED) ──
    let completion_c = if stats.completion_rate > 70.0 { GREEN } else if stats.completion_rate > 50.0 { YELLOW } else { RED };
    let ctx_c = if stats.avg_context_fill < 50.0 { GREEN } else if stats.avg_context_fill < 75.0 { YELLOW } else { RED };

    // Total context tokens used across all cursor sessions
    let total_ctx_tokens: u64 = sorted.iter()
        .filter_map(|s| s.context_tokens_used)
        .sum();
    let avg_ctx_tokens = if stats.total_sessions > 0 {
        total_ctx_tokens / stats.total_sessions as u64
    } else { 0 };

    let kpi_line1 = Line::from(vec![
        Span::styled("   completion ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.0}%", stats.completion_rate), Style::default().fg(completion_c).bold()),
        Span::styled("   lines shipped ", Style::default().fg(FG_FAINT)),
        Span::styled(compact(stats.total_lines), Style::default().fg(FG).bold()),
        Span::styled("   ctx ", Style::default().fg(FG_FAINT)),
        Span::styled(
            format!("{} total  {}/sess  avg {:.0}%",
                compact(total_ctx_tokens), compact(avg_ctx_tokens), stats.avg_context_fill),
            Style::default().fg(ctx_c).bold(),
        ),
        Span::styled("   agent ", Style::default().fg(FG_FAINT)),
        Span::styled(format!("{:.0}%", stats.agent_pct), Style::default().fg(PURPLE).bold()),
    ]);

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

    let spark_str = spark(&stats.monthly_volumes);
    let spark_line = Line::from(vec![
        Span::styled("   7mo ", Style::default().fg(FG_FAINT)),
        Span::styled(spark_str, Style::default().fg(BLUE)),
        Span::styled(format!("  {:.0} lines/sess  {} files shipped",
            stats.avg_lines_per_session, compact(stats.total_files)),
            Style::default().fg(FG_FAINT)),
    ]);

    frame.render_widget(Paragraph::new(vec![kpi_line1, model_line, spark_line]), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // ── Session table using ratatui Table for proper column alignment ──
    let max_rows = (chunks[5].height + chunks[6].height) as usize;

    if state.cursor >= state.scroll + max_rows.saturating_sub(1) {
        state.scroll = state.cursor.saturating_sub(max_rows.saturating_sub(2));
    }
    if state.cursor < state.scroll {
        state.scroll = state.cursor;
    }

    let visible_sessions: Vec<&SessionMeta> = sorted.iter()
        .skip(state.scroll)
        .take(max_rows.saturating_sub(1))
        .copied()
        .collect();

    let header_cells = ["SESSION", "MODEL", "DUR", "COST", "CTX", "STATUS", "AGE", "MODE"]
        .iter()
        .map(|h| Cell::from(Span::styled(*h, Style::default().fg(FG_FAINT))));
    let header_row = Row::new(header_cells);

    let mut table_rows: Vec<Row> = Vec::new();
    for (vi, session) in visible_sessions.iter().enumerate() {
        let i = vi + state.scroll;
        let is_selected = i == state.cursor;
        let is_live = live_sessions.get(&session.session_id).copied().unwrap_or(false);

        let dur = session.duration_minutes();
        let dur_str = if dur >= 60 { format!("{}h{:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };

        let cost = store.session_cost(&session.session_id);
        let cost_str = pricing::format_cost(cost);

        let model = store.session_model(&session.session_id);
        let model_short = truncate_model(&model, 12);

        // CTX: mini-bar from context usage percentage
        let ctx_pct = match (session.context_tokens_used, session.context_token_limit) {
            (Some(used), Some(limit)) if limit > 0 => {
                (used as f64 / limit as f64 * 100.0).min(100.0)
            }
            _ => session.context_usage_pct.unwrap_or(0.0),
        };

        // STATUS
        let (status_text, status_color) = if is_live {
            let analysis_opt = store.analyze_session(&session.session_id);
            if let Some(ref a) = analysis_opt {
                let ceiling = session.context_token_limit;
                let hs = analysis::health_status(a, ceiling, true, config.context_warn_pct, config.context_danger_pct);
                (hs.label().to_string(), health_color(&hs))
            } else {
                ("active".to_string(), GREEN)
            }
        } else {
            match session.cursor_status {
                Some(SessionStatus::Completed) => ("done".to_string(), FG_FAINT),
                Some(SessionStatus::Aborted) => ("aborted".to_string(), RED),
                _ => ("done".to_string(), FG_FAINT),
            }
        };

        // AGE
        let age_str = if is_live {
            let elapsed = (now - session.start_time).num_minutes();
            if elapsed >= 60 { format!("{}h{:02}m", elapsed / 60, elapsed % 60) } else { format!("{}m", elapsed.max(1)) }
        } else {
            format_ago(session.end_time)
        };

        // MODE
        let mode_str = match session.cursor_mode {
            Some(SessionMode::Agent) => "agent",
            Some(SessionMode::Chat) => "chat",
            Some(SessionMode::Plan) => "plan",
            _ => "",
        };

        let fg = if is_selected { FG } else { FG_MUTED };
        let cursor_char = if is_selected { "\u{25b8} " } else { "  " };

        let cells = vec![
            Cell::from(Line::from(vec![
                Span::styled(cursor_char.to_string(), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
                Span::styled(session.first_message.clone(), Style::default().fg(fg)),
            ])),
            Cell::from(Span::styled(model_short, Style::default().fg(FG_FAINT))),
            Cell::from(Span::styled(dur_str, Style::default().fg(FG_FAINT))),
            Cell::from(Span::styled(cost_str, Style::default().fg(FG_MUTED))),
            Cell::from(Line::from(mini_bar(ctx_pct))),
            Cell::from(Span::styled(status_text, Style::default().fg(status_color))),
            Cell::from(Span::styled(age_str, Style::default().fg(FG_FAINT))),
            Cell::from(Span::styled(mode_str.to_string(), Style::default().fg(if mode_str == "agent" { PURPLE } else { FG_FAINT }))),
        ];

        table_rows.push(Row::new(cells));
    }

    if sorted.is_empty() {
        let msg = if state.search_query.is_empty() { "no cursor sessions found" } else { "no matching sessions" };
        table_rows.push(Row::new(vec![Cell::from(Span::styled(msg, Style::default().fg(FG_FAINT)))]));
    }

    let widths = [
        Constraint::Min(20),          // SESSION (fills remaining)
        Constraint::Length(12),        // MODEL
        Constraint::Length(8),         // DUR
        Constraint::Length(8),         // COST
        Constraint::Length(5),         // CTX (mini-bar)
        Constraint::Length(8),         // STATUS
        Constraint::Length(9),         // AGE
        Constraint::Length(6),         // MODE
    ];

    let table = Table::new(table_rows, widths)
        .header(header_row.style(Style::default().fg(FG_FAINT)))
        .column_spacing(1);

    // Merge the header and list areas for the table
    let table_rect = Rect {
        x: chunks[5].x,
        y: chunks[5].y,
        width: chunks[5].width,
        height: chunks[5].height + chunks[6].height,
    };
    frame.render_widget(table, table_rect);
    frame.render_widget(Paragraph::new(divider(w)), chunks[7]);

    // ── Help bar or search input ──
    if state.search_active {
        let search_line = Line::from(vec![
            Span::styled("   /", Style::default().fg(ACCENT)),
            Span::styled(format!("{}_", state.search_query), Style::default().fg(FG)),
        ]);
        frame.render_widget(Paragraph::new(search_line), chunks[8]);
    } else if !state.search_query.is_empty() {
        let help = help_bar(&[
            ("\u{2191}\u{2193}", "navigate"),
            ("enter", "detail"),
            ("esc", "clear search"),
            ("s", "sort"),
            ("/", "search"),
            ("q", "quit"),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[8]);
    } else {
        let help = help_bar(&[
            ("\u{2191}\u{2193}", "navigate"),
            ("enter", "detail"),
            ("esc", "back"),
            ("s", "sort"),
            ("/", "search"),
            ("d", "claude code"),
            ("q", "quit"),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[8]);
    }
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

fn cursor_status_order(session: &SessionMeta, live_sessions: &std::collections::HashMap<String, bool>) -> u8 {
    if live_sessions.get(&session.session_id).copied().unwrap_or(false) {
        return 10; // live sessions first
    }
    match session.cursor_status {
        Some(SessionStatus::Aborted) => 5,
        Some(SessionStatus::Completed) => 1,
        _ => 1,
    }
}
