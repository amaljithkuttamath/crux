use crate::config::Config;
use crate::parser::conversation::{self, ConversationMessage};
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use chrono::{Datelike, Utc};
use ratatui::prelude::*;
use ratatui::widgets::*;

#[derive(Default)]
pub struct SessionsState {
    pub cursor: usize,
    pub scroll: usize,
    pub detail: Option<SessionDetail>,
    pub detail_scroll: usize,
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

pub fn render(frame: &mut ratatui::Frame, store: &Store, config: &Config, state: &mut SessionsState) {
    if state.detail.is_some() {
        render_detail(frame, store, config, state);
    } else {
        render_list(frame, store, config, state);
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Claude Code full view: daily cost BarChart + model breakdown + sessions
// ════════════════════════════════════════════════════════════════════════

fn render_list(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &mut SessionsState) {
    let area = frame.area();
    let w = area.width;
    let sessions = store.sessions_by_source(crate::parser::Source::ClaudeCode);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),   // title
            Constraint::Length(1),   // divider
            Constraint::Length(4),   // daily cost bars + model breakdown
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // session list
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help
        ])
        .split(area);

    // ── Title ──
    let session_count = sessions.len();
    let total_cost: f64 = sessions.iter()
        .map(|s| store.session_cost(&s.session_id))
        .sum();
    let today = store.today_by_source(crate::parser::Source::ClaudeCode);
    let title = Line::from(vec![
        Span::styled("   ", Style::default()),
        Span::styled("\u{25cf}", Style::default().fg(ACCENT2)),
        Span::styled(" Claude Code", Style::default().fg(FG).bold()),
        Span::styled(
            format!("{}{} today  {} total  {} sessions",
                " ".repeat((w as usize).saturating_sub(60).max(1)),
                pricing::format_cost(today.cost),
                pricing::format_cost(total_cost),
                session_count,
            ),
            Style::default().fg(FG_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Top zone: 7-day cost bars + model split ──
    let days = store.by_day(7);
    let max_cost = days.iter().map(|d| d.cost).fold(0.0f64, f64::max).max(0.01);
    let bar_w = 10usize;

    let mut top_lines: Vec<Line> = Vec::new();

    // Daily cost bars (last 7 days, horizontal)
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

    // Model breakdown on remaining lines
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
    frame.render_widget(Paragraph::new(top_lines), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // ── Session list with date grouping ──
    let max_rows = chunks[4].height as usize;
    let mut lines: Vec<Line> = Vec::new();
    let mut current_date_label = String::new();
    let now = Utc::now();

    if state.cursor >= state.scroll + max_rows {
        state.scroll = state.cursor.saturating_sub(max_rows - 1);
    }
    if state.cursor < state.scroll { state.scroll = state.cursor; }

    let mut row_idx = 0usize;
    let mut visible_row = 0usize;

    for (i, session) in sessions.iter().enumerate() {
        let date = session.start_time.date_naive();
        let label = if date == now.date_naive() { "TODAY".to_string() }
            else if date == (now - chrono::Duration::days(1)).date_naive() { "YESTERDAY".to_string() }
            else { format!("{} {}", month_abbrev(date.month()), date.day()) };

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
        let analysis = store.analyze_session(&session.session_id);
        let cost = analysis.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
        let duration = session.duration_minutes();
        let dur_str = if duration >= 60 { format!("{}h{:02}m", duration / 60, duration % 60) } else { format!("{}m", duration.max(1)) };
        let time_str = session.start_time.format("%H:%M").to_string();

        let ctx_pct = analysis.as_ref().map(|a| (a.context_current as f64 / 167_000.0 * 100.0).min(100.0)).unwrap_or(0.0);
        let ctx_indicator = if ctx_pct > 85.0 { ("\u{25cf}", RED) }
            else if ctx_pct > 60.0 { ("\u{25d0}", YELLOW) }
            else { ("\u{25cb}", FG_FAINT) };

        let tool_calls: usize = session.tool_counts.values().sum();
        let tool_heavy = tool_calls as f64 / session.message_count.max(1) as f64 > 0.5;
        let type_badge = if session.agent_spawns > 2 { "agt" }
            else if tool_heavy { "tool" }
            else { "chat" };

        let grade = analysis.as_ref().map(|a| a.grade_letter()).unwrap_or("-");
        let grade_color = match grade { "A" => GREEN, "B" => ACCENT, "C" => YELLOW, _ => RED };

        let (fg_main, fg_sub) = if is_selected { (FG, ACCENT) } else { (FG_MUTED, FG_FAINT) };
        let cursor_char = if is_selected { "\u{25b8}" } else { " " };

        // Context sparkline from timeline
        let ctx_spark = store.session_timeline(&session.session_id)
            .map(|tl| {
                let vals: Vec<f64> = tl.turns.iter().map(|t| t.context_pct).collect();
                spark(&vals)
            })
            .unwrap_or_default();

        let top_tools: String = session.tools_used.iter().take(2)
            .map(|t| { let c = session.tool_counts.get(t).unwrap_or(&0); format!("{}({})", shorten_tool(t), c) })
            .collect::<Vec<_>>().join(" ");

        let name_w = (w as usize).saturating_sub(75).max(8);
        let topic = truncate(&session.first_message, name_w);

        lines.push(Line::from(vec![
            Span::styled(format!("  {}", cursor_char), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(ctx_indicator.0, Style::default().fg(ctx_indicator.1)),
            Span::styled(format!(" {} ", time_str), Style::default().fg(fg_sub)),
            Span::styled(format!("{:<width$}", topic, width = name_w), Style::default().fg(fg_main)),
            Span::styled(format!(" {:>4}", dur_str), Style::default().fg(fg_sub)),
            Span::styled(format!("  {:>7}", pricing::format_cost(cost)), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(format!(" {}", ctx_spark), Style::default().fg(ctx_indicator.1)),
            Span::styled(format!("{:>3.0}%", ctx_pct), Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {}", grade), Style::default().fg(grade_color)),
            Span::styled(format!("  {:<4}", type_badge), Style::default().fg(if type_badge == "agt" { PURPLE } else { FG_FAINT })),
            Span::styled(top_tools, Style::default().fg(FG_FAINT)),
        ]));

        visible_row += 1;
        row_idx += 1;
    }

    frame.render_widget(Paragraph::new(lines), chunks[4]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[5]);

    let help = help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("enter", "detail"),
        ("esc", "back"),
        ("c", "cursor"),
        ("h", "history"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[6]);
}

// ════════════════════════════════════════════════════════════════════════
//  Session detail view
// ════════════════════════════════════════════════════════════════════════

fn render_detail(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &mut SessionsState) {
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

        let grade = analysis.as_ref().map(|a| a.grade_letter()).unwrap_or("-");
        let grade_c = match grade { "A" => GREEN, "B" => ACCENT, "C" => YELLOW, _ => RED };

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
                Span::styled(format!("  {}", grade), Style::default().fg(grade_c).bold()),
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        // ── Stats: context + tools + cost breakdown ──
        let mut stat_lines: Vec<Line> = Vec::new();

        if let Some(a) = &analysis {
            let ctx_pct = (a.context_current as f64 / 167_000.0 * 100.0).min(100.0);
            let ctx_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { GREEN };
            let bar_w = 15usize;
            let (bf, be) = smooth_bar(ctx_pct, 100.0, bar_w);

            stat_lines.push(Line::from(vec![
                Span::styled("   ctx  ", Style::default().fg(FG_FAINT)),
                Span::styled(bf, Style::default().fg(ctx_color)),
                Span::styled(be, Style::default().fg(FG_FAINT)),
                Span::styled(format!(" {:.0}%", ctx_pct), Style::default().fg(ctx_color).bold()),
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
