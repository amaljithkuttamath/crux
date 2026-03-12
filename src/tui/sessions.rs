use crate::config::Config;
use crate::parser::conversation::{self, ConversationMessage};
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;
use chrono::{Datelike, Utc};

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
        if self.detail.is_some() {
            return;
        }
        let sessions = store.sessions_by_time();
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
        if self.detail.is_some() {
            self.detail = None;
            true
        } else {
            false
        }
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
//  Session list
// ════════════════════════════════════════════════════════════════════════

fn render_list(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &mut SessionsState) {
    let area = frame.area();
    let w = area.width;
    let sessions = store.sessions_by_time();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),   // title
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

    let title = Line::from(vec![
        Span::styled("   sessions", Style::default().fg(ACCENT).bold()),
        Span::styled(
            format!("{}{}  {}",
                " ".repeat((w as usize).saturating_sub(35)),
                session_count,
                pricing::format_cost(total_cost),
            ),
            Style::default().fg(FG_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(title), chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // ── Session list with date grouping ──
    let max_rows = chunks[2].height as usize;
    let mut lines: Vec<Line> = Vec::new();
    let mut current_date_label = String::new();
    let now = Utc::now();

    // Keep cursor in view
    if state.cursor >= state.scroll + max_rows {
        state.scroll = state.cursor.saturating_sub(max_rows - 1);
    }
    if state.cursor < state.scroll {
        state.scroll = state.cursor;
    }

    let mut row_idx = 0usize;
    let mut visible_row = 0usize;

    for (i, session) in sessions.iter().enumerate() {
        // Date grouping
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

        if row_idx < state.scroll {
            row_idx += 1;
            continue;
        }
        if visible_row >= max_rows { break; }

        let is_selected = i == state.cursor;
        let analysis = store.analyze_session(&session.session_id);
        let cost = analysis.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
        let duration = session.duration_minutes();
        let dur_str = if duration >= 60 {
            format!("{}h{:02}m", duration / 60, duration % 60)
        } else {
            format!("{}m", duration.max(1))
        };

        let time_str = session.start_time.format("%H:%M").to_string();

        // Context fill indicator
        let ctx_pct = analysis.as_ref().map(|a| {
            (a.context_current as f64 / 167_000.0 * 100.0).min(100.0)
        }).unwrap_or(0.0);

        let ctx_indicator = if ctx_pct > 85.0 {
            ("\u{25cf}", RED) // filled circle - critical
        } else if ctx_pct > 60.0 {
            ("\u{25d0}", YELLOW) // half circle - warning
        } else {
            ("\u{25cb}", FG_FAINT) // empty circle - fine
        };

        // Session type
        let total_msgs = session.message_count.max(1);
        let tool_calls: usize = session.tool_counts.values().sum();
        let type_badge = if session.agent_spawns > 2 {
            "agt"
        } else if tool_calls as f64 / total_msgs as f64 > 0.5 {
            "tool"
        } else {
            "chat"
        };

        // Grade
        let grade = analysis.as_ref().map(|a| grade_letter(a)).unwrap_or("-");
        let grade_color = match grade {
            "A" => Color::Rgb(120, 190, 120),
            "B" => ACCENT,
            "C" => YELLOW,
            _ => RED,
        };

        let (fg_main, fg_sub) = if is_selected {
            (FG, ACCENT)
        } else {
            (FG_MUTED, FG_FAINT)
        };

        let cursor_char = if is_selected { "\u{25b8}" } else { " " };

        // Top tools summary (2 most used)
        let top_tools: String = session.tools_used.iter()
            .take(2)
            .map(|t| {
                let count = session.tool_counts.get(t).unwrap_or(&0);
                format!("{}({})", shorten_tool(t), count)
            })
            .collect::<Vec<_>>()
            .join(" ");

        let name_w = (w as usize).saturating_sub(70).max(8);
        let topic = truncate(&session.first_message, name_w);

        // Line 1: time, topic, cost, grade, context indicator
        lines.push(Line::from(vec![
            Span::styled(format!("  {}", cursor_char), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(ctx_indicator.0, Style::default().fg(ctx_indicator.1)),
            Span::styled(format!(" {} ", time_str), Style::default().fg(fg_sub)),
            Span::styled(format!("{:<width$}", topic, width = name_w), Style::default().fg(fg_main)),
            Span::styled(format!(" {:>4}", dur_str), Style::default().fg(fg_sub)),
            Span::styled(format!("  {}m", session.user_count), Style::default().fg(fg_sub)),
            Span::styled(format!("  {:>7}", pricing::format_cost(cost)), Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(format!("  {}", grade), Style::default().fg(grade_color)),
            Span::styled(format!("  {:<4}", type_badge), Style::default().fg(FG_FAINT)),
            Span::styled(top_tools, Style::default().fg(FG_FAINT)),
        ]));

        visible_row += 1;
        row_idx += 1;
    }

    frame.render_widget(Paragraph::new(lines), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    let help = help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("enter", "detail"),
        ("esc", "back"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[4]);
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
            Constraint::Length(2),   // header
            Constraint::Length(3),   // stats
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // conversation timeline
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help
        ])
        .split(area);

    // ── Header ──
    if let Some(meta) = meta {
        let cost = analysis.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
        let dur = meta.duration_minutes();
        let dur_str = if dur >= 60 {
            format!("{}h{:02}m", dur / 60, dur % 60)
        } else {
            format!("{}m", dur.max(1))
        };
        let date_str = meta.start_time.format("%Y-%m-%d %H:%M").to_string();

        let grade = analysis.as_ref().map(|a| grade_letter(a)).unwrap_or("-");
        let grade_c = match grade {
            "A" => Color::Rgb(120, 190, 120),
            "B" => ACCENT,
            "C" => YELLOW,
            _ => RED,
        };

        let header = vec![
            Line::from(vec![
                Span::styled(format!("   {}", truncate(&meta.first_message, (w as usize).saturating_sub(10))),
                    Style::default().fg(FG).bold()),
            ]),
            Line::from(vec![
                Span::styled(format!("   {}", date_str), Style::default().fg(ACCENT)),
                Span::styled(format!("  {}", meta.project), Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {}  {}  {}m turns", dur_str, pricing::format_cost(cost), meta.user_count),
                    Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {}", grade), Style::default().fg(grade_c).bold()),
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        // ── Stats block: context + tools + cost breakdown ──
        let mut stat_lines: Vec<Line> = Vec::new();

        if let Some(a) = &analysis {
            // Context trajectory + cache
            let ctx_pct = (a.context_current as f64 / 167_000.0 * 100.0).min(100.0);
            let ctx_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { Color::Rgb(120, 190, 120) };
            let bar_w = 15usize;
            let (bf, be) = smooth_bar(ctx_pct, 100.0, bar_w);

            stat_lines.push(Line::from(vec![
                Span::styled("   ctx  ", Style::default().fg(FG_FAINT)),
                Span::styled(bf, Style::default().fg(ctx_color)),
                Span::styled(be, Style::default().fg(FG_FAINT)),
                Span::styled(format!(" {:.0}%", ctx_pct), Style::default().fg(ctx_color).bold()),
                Span::styled(
                    format!("  {} \u{2192} {}", compact(a.context_initial), compact(a.context_current)),
                    Style::default().fg(FG_MUTED),
                ),
                Span::styled(format!("  {:.1}x growth", a.context_growth), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  cache {:.0}%", a.cache_hit_rate * 100.0), Style::default().fg(FG_FAINT)),
                if a.compaction_count > 0 {
                    Span::styled(
                        format!("  {} compactions", a.compaction_count),
                        Style::default().fg(YELLOW),
                    )
                } else {
                    Span::raw("")
                },
            ]));

            // Cost breakdown
            let cb = &a.cost_breakdown;
            stat_lines.push(Line::from(vec![
                Span::styled("   cost ", Style::default().fg(FG_FAINT)),
                Span::styled(
                    format!("out {}  in {}  cache-r {}  cache-w {}",
                        pricing::format_cost(cb.output),
                        pricing::format_cost(cb.input),
                        pricing::format_cost(cb.cache_read),
                        pricing::format_cost(cb.cache_write),
                    ),
                    Style::default().fg(FG_MUTED),
                ),
                Span::styled(
                    format!("  {}/1K out", pricing::format_cost(a.cost_per_1k_output)),
                    Style::default().fg(FG_FAINT),
                ),
            ]));
        }

        // Tool usage
        let top_tools: Vec<String> = meta.tools_used.iter()
            .take(8)
            .map(|t| {
                let count = meta.tool_counts.get(t).unwrap_or(&0);
                format!("{}({})", t, count)
            })
            .collect();
        if !top_tools.is_empty() {
            stat_lines.push(Line::from(vec![
                Span::styled("   tools ", Style::default().fg(FG_FAINT)),
                Span::styled(top_tools.join("  "), Style::default().fg(FG_MUTED)),
                if meta.agent_spawns > 0 {
                    Span::styled(
                        format!("  {} agents spawned", meta.agent_spawns),
                        Style::default().fg(YELLOW),
                    )
                } else {
                    Span::raw("")
                },
            ]));
        }

        frame.render_widget(Paragraph::new(stat_lines), chunks[1]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── Conversation timeline ──
    // Interleave user messages with context snapshots from timeline
    let max_rows = chunks[3].height as usize;

    // Build unified timeline: user messages + notable context events
    let mut timeline_entries: Vec<TimelineEntry> = Vec::new();

    // Add user messages
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

    // Add notable context events from timeline
    if let Some(ref tl) = timeline {
        let thresholds = [50.0, 75.0, 85.0];
        let mut last_crossed: Option<usize> = None;

        for turn in &tl.turns {
            let current_threshold = thresholds.iter().rposition(|&t| turn.context_pct >= t);
            let crossed_new = current_threshold != last_crossed;
            if crossed_new { last_crossed = current_threshold; }

            if turn.is_compaction {
                timeline_entries.push(TimelineEntry {
                    timestamp: turn.timestamp,
                    kind: EntryKind::Compaction(turn.context_pct),
                });
            } else if turn.cost > 0.50 {
                timeline_entries.push(TimelineEntry {
                    timestamp: turn.timestamp,
                    kind: EntryKind::CostSpike(turn.cost),
                });
            } else if crossed_new && turn.context_pct >= 75.0 {
                timeline_entries.push(TimelineEntry {
                    timestamp: turn.timestamp,
                    kind: EntryKind::ContextWarning(turn.context_pct),
                });
            }
        }
    }

    // Sort by timestamp, deduplicate close events
    timeline_entries.sort_by_key(|e| e.timestamp);

    // Clamp scroll
    let max_scroll = timeline_entries.len().saturating_sub(max_rows);
    if state.detail_scroll > max_scroll {
        state.detail_scroll = max_scroll;
    }

    let mut lines: Vec<Line> = Vec::new();
    for entry in timeline_entries.iter().skip(state.detail_scroll).take(max_rows) {
        let time_str = entry.timestamp.format("%H:%M").to_string();
        let content_w = (w as usize).saturating_sub(14).max(10);

        match &entry.kind {
            EntryKind::UserMessage(content) => {
                let text = truncate(content, content_w);
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled("\u{25b8} ", Style::default().fg(ACCENT)),
                    Span::styled(text, Style::default().fg(FG)),
                ]));
            }
            EntryKind::ToolUse(tools) => {
                let tool_str = tools.iter()
                    .take(4)
                    .map(|t| shorten_tool(t))
                    .collect::<Vec<_>>()
                    .join(" ");
                let extra = if tools.len() > 4 {
                    format!(" +{}", tools.len() - 4)
                } else {
                    String::new()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled("\u{2192} ", Style::default().fg(FG_FAINT)),
                    Span::styled(format!("{}{}", tool_str, extra), Style::default().fg(FG_FAINT)),
                ]));
            }
            EntryKind::Compaction(pct) => {
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled(
                        format!("\u{2193} compacted to {:.0}%", pct),
                        Style::default().fg(YELLOW).bold(),
                    ),
                ]));
            }
            EntryKind::CostSpike(cost) => {
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled(
                        format!("\u{26a0} cost spike {}", pricing::format_cost(*cost)),
                        Style::default().fg(YELLOW),
                    ),
                ]));
            }
            EntryKind::ContextWarning(pct) => {
                let color = if *pct > 85.0 { RED } else { YELLOW };
                lines.push(Line::from(vec![
                    Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
                    Span::styled(
                        format!("\u{25b2} context {:.0}%", pct),
                        Style::default().fg(color),
                    ),
                ]));
            }
        }
    }

    // Scroll indicator
    if timeline_entries.len() > max_rows {
        let pct = if max_scroll > 0 {
            state.detail_scroll as f64 / max_scroll as f64 * 100.0
        } else { 0.0 };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}[{:.0}%]", " ".repeat((w as usize).saturating_sub(8)), pct),
                Style::default().fg(FG_FAINT),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    let entry_count = timeline_entries.len();
    let help = help_bar(&[
        ("\u{2191}\u{2193}", "scroll"),
        ("esc", "back"),
        ("q", "quit"),
        ("", &format!("{} events", entry_count)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[5]);
}

// ════════════════════════════════════════════════════════════════════════
//  Timeline entry types
// ════════════════════════════════════════════════════════════════════════

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

/// Shorten tool names for compact display
fn shorten_tool(name: &str) -> String {
    match name {
        "Read" => "Rd".to_string(),
        "Write" => "Wr".to_string(),
        "Edit" => "Ed".to_string(),
        "Bash" => "Sh".to_string(),
        "Glob" => "Gl".to_string(),
        "Grep" => "Gr".to_string(),
        "Agent" => "Ag".to_string(),
        "Skill" => "Sk".to_string(),
        "WebFetch" => "WF".to_string(),
        "WebSearch" => "WS".to_string(),
        "NotebookEdit" => "NE".to_string(),
        _ => {
            if name.len() > 4 {
                name[..4].to_string()
            } else {
                name.to_string()
            }
        }
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

fn month_abbrev(month: u32) -> &'static str {
    match month {
        1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR",
        5 => "MAY", 6 => "JUN", 7 => "JUL", 8 => "AUG",
        9 => "SEP", 10 => "OCT", 11 => "NOV", 12 => "DEC",
        _ => "???",
    }
}
