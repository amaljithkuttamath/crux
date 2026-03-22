use crate::config::Config;
use crate::parser::conversation::{self, ConversationMessage};
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
    Cost,
    Age,
    Ctx,
    Status,
    Duration,
}

impl SortColumn {
    pub fn next(&self) -> Self {
        match self {
            Self::Cost => Self::Age,
            Self::Age => Self::Ctx,
            Self::Ctx => Self::Status,
            Self::Status => Self::Duration,
            Self::Duration => Self::Cost,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cost => "cost",
            Self::Age => "age",
            Self::Ctx => "ctx",
            Self::Status => "status",
            Self::Duration => "duration",
        }
    }
}

#[derive(Default)]
pub struct SessionsState {
    pub cursor: usize,
    pub scroll: usize,
    pub detail: Option<SessionDetail>,
    pub detail_scroll: usize,
    pub sort_column: SortColumn,
    pub search_active: bool,
    pub search_query: String,
}

pub struct SessionDetail {
    pub session_id: String,
    pub messages: Vec<ConversationMessage>,
}

impl SessionsState {
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
        let sessions = store.sessions_by_source(crate::parser::Source::ClaudeCode);
        if let Some(session) = sessions.get(self.cursor) {
            if let Ok(messages) = conversation::parse_conversation(&session.file_path) {
                self.detail = Some(SessionDetail {
                    session_id: session.session_id.clone(),
                    messages,
                });
                self.detail_scroll = 0;
            }
        }
    }

    pub fn back(&mut self) -> bool {
        if self.detail.is_some() { self.detail = None; true } else { false }
    }
}

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut SessionsState, live_sessions: &std::collections::HashMap<String, bool>) {
    if state.detail.is_some() {
        render_detail(frame, store, config, state);
    } else {
        render_list(frame, store, config, state, live_sessions);
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Claude Code full view: daily cost BarChart + model breakdown + sessions
// ════════════════════════════════════════════════════════════════════════

fn render_list(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut SessionsState, live_sessions: &std::collections::HashMap<String, bool>) {
    let area = frame.area();
    let w = area.width;
    let all_sessions = store.sessions_by_source(crate::parser::Source::ClaudeCode);

    // Filter to non-subagent parent sessions (subagents rendered inline)
    let parent_sessions: Vec<&&crate::parser::conversation::SessionMeta> = all_sessions.iter()
        .filter(|s| !s.is_subagent)
        .collect();

    // Apply search filter
    let query_lower = state.search_query.to_lowercase();
    let filtered: Vec<&&crate::parser::conversation::SessionMeta> = if state.search_query.is_empty() {
        parent_sessions
    } else {
        parent_sessions.into_iter()
            .filter(|s| s.first_message.to_lowercase().contains(&query_lower))
            .collect()
    };

    // Partition into active vs completed
    let mut active: Vec<(&crate::parser::conversation::SessionMeta, Option<crate::store::SessionAnalysis>)> = Vec::new();
    let mut completed: Vec<(&crate::parser::conversation::SessionMeta, Option<crate::store::SessionAnalysis>)> = Vec::new();

    for s in &filtered {
        let is_live = live_sessions.get(&s.session_id).copied().unwrap_or(false);
        let ana = store.analyze_session(&s.session_id);
        if is_live {
            active.push((s, ana));
        } else {
            completed.push((s, ana));
        }
    }

    // Sort within groups
    sort_sessions(&mut active, state.sort_column, config.context_warn_pct, config.context_danger_pct);
    sort_sessions(&mut completed, state.sort_column, config.context_warn_pct, config.context_danger_pct);

    let active_count = active.len();
    let session_count = filtered.len();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),   // nav header
            Constraint::Length(1),   // source header
            Constraint::Length(1),   // divider
            Constraint::Length(4),   // daily cost bars + model breakdown
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // column headers
            Constraint::Min(4),     // session list
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help / search
        ])
        .split(area);

    // ── Nav header ──
    let nav = nav_header("claude_code", w);
    frame.render_widget(Paragraph::new(nav), chunks[0]);

    // ── Source header ──
    let today = store.today_by_source(crate::parser::Source::ClaudeCode);
    let source_header = Line::from(vec![
        Span::styled("   ", Style::default()),
        Span::styled("\u{25cf}", Style::default().fg(ACCENT2)),
        Span::styled(" Claude Code", Style::default().fg(FG).bold()),
        Span::styled(
            format!("  {} today  {} sessions  {} active",
                pricing::format_cost(today.cost),
                session_count,
                active_count,
            ),
            Style::default().fg(FG_MUTED),
        ),
        Span::styled(
            format!("{}sort: {}\u{25bc}",
                " ".repeat((w as usize).saturating_sub(65).max(1)),
                state.sort_column.label(),
            ),
            Style::default().fg(FG_FAINT),
        ),
    ]);
    frame.render_widget(Paragraph::new(source_header), chunks[1]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── Top zone: 7-day cost bars + model split (PRESERVED) ──
    let days = store.by_day(7);
    let max_cost = days.iter().map(|d| d.cost).fold(0.0f64, f64::max).max(0.01);
    let bar_w = 10usize;

    let mut top_lines: Vec<Line> = Vec::new();

    let today_date = chrono::Utc::now().date_naive();
    for day in days.iter().take(4) {
        let is_today = day.date == today_date;
        let (bf, be) = smooth_bar(day.cost, max_cost, bar_w);
        let label = if is_today { "today".to_string() } else { day.date.format("%a").to_string() };
        let bar_color = if is_today { ACCENT } else { FG_MUTED };

        top_lines.push(Line::from(vec![
            Span::styled(format!("   {:<6}", label), Style::default().fg(if is_today { FG } else { FG_FAINT })),
            Span::styled(bf, Style::default().fg(bar_color)),
            Span::styled(be, Style::default().fg(FG_FAINT)),
            Span::styled(format!(" {:>7}  {} sess", pricing::format_cost(day.cost), day.session_count),
                Style::default().fg(FG_FAINT)),
        ]));
    }

    let models = store.by_model();
    if !models.is_empty() && top_lines.len() < 4 {
        let mut model_spans: Vec<Span> = vec![Span::styled("   models  ", Style::default().fg(FG_FAINT))];
        let total_model_cost: f64 = models.iter().map(|m| m.cost).sum();
        for m in models.iter().take(3) {
            let pct = if total_model_cost > 0.0 { m.cost / total_model_cost * 100.0 } else { 0.0 };
            let color = match m.name.as_str() {
                "opus" => PURPLE,
                "sonnet" => ACCENT,
                "haiku" => ACCENT2,
                _ => FG_MUTED,
            };
            model_spans.push(Span::styled(format!("{} ", m.name), Style::default().fg(color)));
            model_spans.push(Span::styled(format!("{:.0}%  ", pct), Style::default().fg(FG_FAINT)));
        }
        top_lines.push(Line::from(model_spans));
    }

    while top_lines.len() < 4 { top_lines.push(Line::from(Span::raw(""))); }
    frame.render_widget(Paragraph::new(top_lines), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // ── Column headers ──
    let name_w = (w as usize).saturating_sub(62).max(8);
    let col_header = Line::from(vec![
        Span::styled(format!("   {:<width$}", "SESSION", width = name_w), Style::default().fg(FG_FAINT)),
        Span::styled("  MODEL ", Style::default().fg(FG_FAINT)),
        Span::styled("  DUR ", Style::default().fg(FG_FAINT)),
        Span::styled("    COST", Style::default().fg(FG_FAINT)),
        Span::styled("  CTX       ", Style::default().fg(FG_FAINT)),
        Span::styled(" STATUS ", Style::default().fg(FG_FAINT)),
        Span::styled(" AGE", Style::default().fg(FG_FAINT)),
    ]);
    frame.render_widget(Paragraph::new(col_header), chunks[5]);

    // ── Session list: active on top, then completed ──
    let max_rows = chunks[6].height as usize;

    // Build flat list of display rows
    #[allow(dead_code)]
    struct DisplayRow {
        session_id: String,
        is_active: bool,
        is_subagent: bool,
        tree_prefix: String,
        topic: String,
        model_str: String,
        dur_str: String,
        cost: f64,
        ctx_str: String,
        status_label: String,
        status_color: Color,
        age_str: String,
    }

    let mut rows: Vec<DisplayRow> = Vec::new();

    let build_rows = |sessions: &[(&crate::parser::conversation::SessionMeta, Option<crate::store::SessionAnalysis>)], is_active_group: bool, rows: &mut Vec<DisplayRow>| {
        for (meta, ana) in sessions {
            let cost = ana.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
            let duration = meta.duration_minutes();
            let dur_str = if duration >= 60 { format!("{}h{:02}", duration / 60, duration % 60) } else { format!("{}m", duration.max(1)) };

            let raw_model = store.session_model(&meta.session_id);
            let model_str = crate::store::simplify_model(&raw_model);

            // CTX display
            let ctx_str = if let Some(limit) = meta.context_token_limit {
                let current = ana.as_ref().map(|a| a.context_current).unwrap_or(0);
                let pct = if limit > 0 { (current as f64 / limit as f64 * 100.0).min(100.0) } else { 0.0 };
                format!("{}/{} {:.0}%", compact(current), compact(limit), pct)
            } else {
                let current = ana.as_ref().map(|a| a.context_current).unwrap_or(0);
                if current > 0 { format!("{} tokens", compact(current)) } else { "--".to_string() }
            };

            // Status
            let is_live = is_active_group;
            let (status_label, status_color) = if let Some(a) = ana {
                let ceiling = meta.context_token_limit;
                let hs = analysis::health_status(a, ceiling, is_live, config.context_warn_pct, config.context_danger_pct);
                (hs.label().to_string(), health_color(&hs))
            } else if is_live {
                ("running".to_string(), GREEN)
            } else {
                ("done".to_string(), FG_FAINT)
            };

            // Age
            let age_str = if is_live {
                let elapsed = (Utc::now() - meta.start_time).num_minutes();
                if elapsed >= 60 { format!("{}h{:02}", elapsed / 60, elapsed % 60) } else { format!("{}m", elapsed.max(1)) }
            } else {
                format_ago(meta.end_time)
            };

            let prefix = if is_active_group { "\u{25b6} ".to_string() } else { "  ".to_string() };

            rows.push(DisplayRow {
                session_id: meta.session_id.clone(),
                is_active: is_active_group,
                is_subagent: false,
                tree_prefix: prefix,
                topic: meta.first_message.clone(),
                model_str,
                dur_str,
                cost,
                ctx_str,
                status_label,
                status_color,
                age_str,
            });

            // Find subagents for this session
            let subagents: Vec<&crate::parser::conversation::SessionMeta> = all_sessions.iter()
                .filter(|s| s.parent_session_id.as_deref() == Some(&meta.session_id))
                .copied()
                .collect();

            for (si, sub) in subagents.iter().enumerate() {
                let is_last = si == subagents.len() - 1;
                let tree_char = if is_last { "\u{2514}\u{2500} " } else { "\u{251c}\u{2500} " };

                let sub_ana = store.analyze_session(&sub.session_id);
                let sub_cost = sub_ana.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
                let sub_dur = sub.duration_minutes();
                let sub_dur_str = if sub_dur >= 60 { format!("{}h{:02}", sub_dur / 60, sub_dur % 60) } else { format!("{}m", sub_dur.max(1)) };
                let sub_model = crate::store::simplify_model(&store.session_model(&sub.session_id));
                let sub_is_live = live_sessions.get(&sub.session_id).copied().unwrap_or(false);
                let sub_status = if sub_is_live { "running" } else { "done" };
                let sub_status_color = if sub_is_live { GREEN } else { FG_FAINT };
                let sub_name = sub.agent_type.clone().unwrap_or_else(|| "subagent".to_string());

                rows.push(DisplayRow {
                    session_id: sub.session_id.clone(),
                    is_active: false,
                    is_subagent: true,
                    tree_prefix: format!("  {}", tree_char),
                    topic: sub_name,
                    model_str: sub_model,
                    dur_str: sub_dur_str,
                    cost: sub_cost,
                    ctx_str: "--".to_string(),
                    status_label: sub_status.to_string(),
                    status_color: sub_status_color,
                    age_str: String::new(),
                });
            }
        }
    };

    build_rows(&active, true, &mut rows);

    // Dashed separator between active and completed (only if both non-empty)
    let need_separator = !active.is_empty() && !completed.is_empty();

    build_rows(&completed, false, &mut rows);

    // Adjust cursor bounds (only parent sessions are selectable, excluding subagent rows)
    let selectable_count = rows.iter().filter(|r| !r.is_subagent).count();
    if state.cursor >= selectable_count && selectable_count > 0 {
        state.cursor = selectable_count - 1;
    }

    // Map cursor index to row index (skip subagent rows in cursor counting)
    let mut cursor_row_idx: Option<usize> = None;
    {
        let mut selectable_i = 0usize;
        for (ri, row) in rows.iter().enumerate() {
            if !row.is_subagent {
                if selectable_i == state.cursor {
                    cursor_row_idx = Some(ri);
                    break;
                }
                selectable_i += 1;
            }
        }
    }

    // Scrolling
    let separator_extra = if need_separator { 1 } else { 0 };
    let _total_display_rows = rows.len() + separator_extra;
    if let Some(cri) = cursor_row_idx {
        // Account for separator offset
        let effective_row = if need_separator && cri >= active.len() { cri + 1 } else { cri };
        if effective_row >= state.scroll + max_rows {
            state.scroll = effective_row.saturating_sub(max_rows - 1);
        }
        if effective_row < state.scroll {
            state.scroll = effective_row;
        }
    }

    // Render rows
    let mut lines: Vec<Line> = Vec::new();
    let mut display_idx = 0usize;
    let mut selectable_i = 0usize;
    let active_row_count = {
        let mut c = 0usize;
        for r in &rows {
            if r.is_active || r.is_subagent { c += 1; } else { break; }
        }
        c
    };

    for (ri, row) in rows.iter().enumerate() {
        // Insert dashed separator between active and completed
        if need_separator && ri == active_row_count {
            if display_idx >= state.scroll && lines.len() < max_rows {
                lines.push(dashed_divider(w));
            }
            display_idx += 1;
        }

        if display_idx < state.scroll {
            display_idx += 1;
            if !row.is_subagent { selectable_i += 1; }
            continue;
        }
        if lines.len() >= max_rows { break; }

        let is_selected = !row.is_subagent && selectable_i == state.cursor;
        let (fg_main, fg_sub) = if is_selected { (FG, ACCENT) } else { (FG_MUTED, FG_FAINT) };

        let topic = truncate(&row.topic, name_w.saturating_sub(row.tree_prefix.chars().count()));

        lines.push(Line::from(vec![
            Span::styled(
                row.tree_prefix.clone(),
                Style::default().fg(if row.is_active { GREEN } else if row.is_subagent { FG_FAINT } else if is_selected { ACCENT } else { FG_FAINT }),
            ),
            Span::styled(format!("{:<width$}", topic, width = name_w.saturating_sub(row.tree_prefix.chars().count())), Style::default().fg(fg_main)),
            Span::styled(format!("  {:>6}", row.model_str), Style::default().fg(fg_sub)),
            Span::styled(format!("  {:>4}", row.dur_str), Style::default().fg(fg_sub)),
            Span::styled(format!("  {:>7}", pricing::format_cost(row.cost)), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(format!("  {:>10}", row.ctx_str), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>7}", row.status_label), Style::default().fg(row.status_color)),
            Span::styled(format!("  {}", row.age_str), Style::default().fg(FG_FAINT)),
        ]));

        display_idx += 1;
        if !row.is_subagent { selectable_i += 1; }
    }

    frame.render_widget(Paragraph::new(lines), chunks[6]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[7]);

    // ── Help bar or search input ──
    if state.search_active || !state.search_query.is_empty() {
        let search_line = Line::from(vec![
            Span::styled("   / ", Style::default().fg(ACCENT)),
            Span::styled(format!("{}_", state.search_query), Style::default().fg(FG)),
        ]);
        frame.render_widget(Paragraph::new(search_line), chunks[8]);
    } else {
        let help = help_bar(&[
            ("\u{2191}\u{2193}", "navigate"),
            ("enter", "detail"),
            ("s", "sort"),
            ("/", "search"),
            ("esc", "back"),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[8]);
    }
}

/// Sort a session list by the given column
fn sort_sessions(
    sessions: &mut Vec<(&crate::parser::conversation::SessionMeta, Option<crate::store::SessionAnalysis>)>,
    col: SortColumn,
    warn_pct: f64,
    danger_pct: f64,
) {
    match col {
        SortColumn::Cost => {
            sessions.sort_by(|a, b| {
                let ca = a.1.as_ref().map(|x| x.total_cost).unwrap_or(0.0);
                let cb = b.1.as_ref().map(|x| x.total_cost).unwrap_or(0.0);
                cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        SortColumn::Age => {
            sessions.sort_by(|a, b| b.0.end_time.cmp(&a.0.end_time));
        }
        SortColumn::Duration => {
            sessions.sort_by(|a, b| b.0.duration_minutes().cmp(&a.0.duration_minutes()));
        }
        SortColumn::Ctx => {
            sessions.sort_by(|a, b| {
                let ca = a.1.as_ref().map(|x| x.context_current).unwrap_or(0);
                let cb = b.1.as_ref().map(|x| x.context_current).unwrap_or(0);
                cb.cmp(&ca)
            });
        }
        SortColumn::Status => {
            sessions.sort_by(|a, b| {
                let sa = a.1.as_ref().map(|x| {
                    analysis::health_status(x, a.0.context_token_limit, false, warn_pct, danger_pct).sort_order()
                }).unwrap_or(0);
                let sb = b.1.as_ref().map(|x| {
                    analysis::health_status(x, b.0.context_token_limit, false, warn_pct, danger_pct).sort_order()
                }).unwrap_or(0);
                sb.cmp(&sa)
            });
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Session detail view
// ════════════════════════════════════════════════════════════════════════

fn render_detail(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut SessionsState) {
    let area = frame.area();
    let w = area.width;
    let detail = match &state.detail {
        Some(d) => d,
        None => return,
    };

    let sessions = store.sessions_by_time();
    let meta = sessions.iter().find(|s| s.session_id == detail.session_id);
    let analysis = store.analyze_session(&detail.session_id);
    let timeline = store.session_timeline(&detail.session_id);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // ── Header ──
    if let Some(meta) = meta {
        let cost = analysis.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
        let dur = meta.duration_minutes();
        let dur_str = if dur >= 60 { format!("{}h{:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };
        let date_str = meta.start_time.format("%Y-%m-%d %H:%M").to_string();

        let ceiling = store.session_meta(&detail.session_id).and_then(|m| m.context_token_limit);
        let health = if let Some(ref a) = analysis {
            let status = analysis::health_status(a, ceiling, false, config.context_warn_pct, config.context_danger_pct);
            (status.label(), health_color(&status))
        } else {
            ("", FG_FAINT)
        };

        let header = vec![
            Line::from(vec![
                Span::styled(format!("   {}", truncate(&meta.first_message, (w as usize).saturating_sub(10))),
                    Style::default().fg(FG).bold()),
            ]),
            Line::from(vec![
                Span::styled(format!("   {}", date_str), Style::default().fg(ACCENT)),
                Span::styled(format!("  {}", display_project_name(&meta.project)), Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {}  {}  {}t", dur_str, pricing::format_cost(cost), meta.user_count),
                    Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {}", health.0), Style::default().fg(health.1).bold()),
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        // ── Stats: context + tools + cost breakdown ──
        let mut stat_lines: Vec<Line> = Vec::new();

        if let Some(a) = &analysis {
            let (ctx_pct, ctx_label) = if let Some(ceil) = ceiling {
                let pct = (a.context_current as f64 / ceil as f64 * 100.0).min(100.0);
                let label = format!("{}/{} {:.0}%", compact(a.context_current), compact(ceil), pct);
                (pct, label)
            } else {
                (0.0, format!("{} tokens", compact(a.context_current)))
            };
            let ctx_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { GREEN };
            let bar_w = 15usize;
            let (bf, be) = smooth_bar(ctx_pct, 100.0, bar_w);

            stat_lines.push(Line::from(vec![
                Span::styled("   ctx  ", Style::default().fg(FG_FAINT)),
                Span::styled(bf, Style::default().fg(ctx_color)),
                Span::styled(be, Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {}", ctx_label), Style::default().fg(ctx_color).bold()),
                Span::styled(format!("  {} \u{2192} {}", compact(a.context_initial), compact(a.context_current)),
                    Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {:.1}x growth", a.context_growth), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  cache {:.0}%", a.cache_hit_rate * 100.0), Style::default().fg(FG_FAINT)),
                if a.compaction_count > 0 {
                    Span::styled(format!("  {} compactions", a.compaction_count), Style::default().fg(YELLOW))
                } else { Span::raw("") },
            ]));

            let cb = &a.cost_breakdown;
            stat_lines.push(Line::from(vec![
                Span::styled("   cost ", Style::default().fg(FG_FAINT)),
                Span::styled(
                    format!("out {}  in {}  cache-r {}  cache-w {}",
                        pricing::format_cost(cb.output), pricing::format_cost(cb.input),
                        pricing::format_cost(cb.cache_read), pricing::format_cost(cb.cache_write)),
                    Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {}/1K out", pricing::format_cost(a.cost_per_1k_output)),
                    Style::default().fg(FG_FAINT)),
            ]));
        }

        let top_tools: Vec<String> = meta.tools_used.iter().take(8)
            .map(|t| { let c = meta.tool_counts.get(t).unwrap_or(&0); format!("{}({})", t, c) })
            .collect();
        if !top_tools.is_empty() {
            stat_lines.push(Line::from(vec![
                Span::styled("   tools ", Style::default().fg(FG_FAINT)),
                Span::styled(top_tools.join("  "), Style::default().fg(FG_MUTED)),
                if meta.agent_spawns > 0 {
                    Span::styled(format!("  {} agents", meta.agent_spawns), Style::default().fg(PURPLE))
                } else { Span::raw("") },
            ]));
        }

        frame.render_widget(Paragraph::new(stat_lines), chunks[1]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── Conversation timeline ──
    let max_rows = chunks[3].height as usize;
    let mut timeline_entries: Vec<TimelineEntry> = Vec::new();

    for msg in &detail.messages {
        if msg.role == "user" {
            timeline_entries.push(TimelineEntry {
                timestamp: msg.timestamp,
                kind: EntryKind::UserMessage(msg.content.clone()),
            });
        } else if msg.role == "assistant" && !msg.tool_names.is_empty() {
            timeline_entries.push(TimelineEntry {
                timestamp: msg.timestamp,
                kind: EntryKind::ToolUse(msg.tool_names.clone()),
            });
        }
    }

    if let Some(ref tl) = timeline {
        let thresholds = [50.0, 75.0, 85.0];
        let mut last_crossed: Option<usize> = None;
        for turn in &tl.turns {
            let current_threshold = thresholds.iter().rposition(|&t| turn.context_pct >= t);
            let crossed_new = current_threshold != last_crossed;
            if crossed_new { last_crossed = current_threshold; }

            if turn.is_compaction {
                timeline_entries.push(TimelineEntry { timestamp: turn.timestamp, kind: EntryKind::Compaction(turn.context_pct) });
            } else if turn.cost > 0.50 {
                timeline_entries.push(TimelineEntry { timestamp: turn.timestamp, kind: EntryKind::CostSpike(turn.cost) });
            } else if crossed_new && turn.context_pct >= 75.0 {
                timeline_entries.push(TimelineEntry { timestamp: turn.timestamp, kind: EntryKind::ContextWarning(turn.context_pct) });
            }
        }
    }

    timeline_entries.sort_by_key(|e| e.timestamp);

    let max_scroll = timeline_entries.len().saturating_sub(max_rows);
    if state.detail_scroll > max_scroll { state.detail_scroll = max_scroll; }

    let mut lines: Vec<Line> = Vec::new();
    for entry in timeline_entries.iter().skip(state.detail_scroll).take(max_rows) {
        let time_str = entry.timestamp.format("%H:%M").to_string();
        let content_w = (w as usize).saturating_sub(14).max(10);

        match &entry.kind {
            EntryKind::UserMessage(content) => {
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled("\u{25b8} ", Style::default().fg(ACCENT)),
                    Span::styled(truncate(content, content_w), Style::default().fg(FG)),
                ]));
            }
            EntryKind::ToolUse(tools) => {
                let tool_str = tools.iter().take(4).map(|t| shorten_tool(t)).collect::<Vec<_>>().join(" ");
                let extra = if tools.len() > 4 { format!(" +{}", tools.len() - 4) } else { String::new() };
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled("\u{2192} ", Style::default().fg(FG_FAINT)),
                    Span::styled(format!("{}{}", tool_str, extra), Style::default().fg(FG_FAINT)),
                ]));
            }
            EntryKind::Compaction(pct) => {
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled(format!("\u{2193} compacted to {:.0}%", pct), Style::default().fg(YELLOW).bold()),
                ]));
            }
            EntryKind::CostSpike(cost) => {
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled(format!("\u{26a0} cost spike {}", pricing::format_cost(*cost)), Style::default().fg(YELLOW)),
                ]));
            }
            EntryKind::ContextWarning(pct) => {
                let color = if *pct > 85.0 { RED } else { YELLOW };
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled(format!("\u{25b2} context {:.0}%", pct), Style::default().fg(color)),
                ]));
            }
        }
    }

    frame.render_widget(Paragraph::new(lines), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    let help = help_bar(&[
        ("\u{2191}\u{2193}", "scroll"),
        ("esc", "back"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[5]);
}

struct TimelineEntry {
    timestamp: chrono::DateTime<Utc>,
    kind: EntryKind,
}

enum EntryKind {
    UserMessage(String),
    ToolUse(Vec<String>),
    Compaction(f64),
    CostSpike(f64),
    ContextWarning(f64),
}
