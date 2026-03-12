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

fn render_list(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &mut SessionsState) {
    let area = frame.area();
    let w = area.width;
    let sessions = store.sessions_by_time();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),   // title
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // session list
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help
        ])
        .split(area);

    // Title
    let session_count = sessions.len();
    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("   sessions", Style::default().fg(ACCENT).bold()),
            Span::styled(
                format!("{}{}  total", " ".repeat((w as usize).saturating_sub(30)), session_count),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ]);
    frame.render_widget(title, chunks[0]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[1]);

    // Session list with date grouping
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

    // Build visible lines
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
        let cost = store.session_cost(&session.session_id);
        let duration = session.duration_minutes();
        let dur_str = if duration >= 60 {
            format!("{}h{:02}m", duration / 60, duration % 60)
        } else {
            format!("{}m", duration.max(1))
        };

        // Session weight indicator
        let weight = if session.user_count > 50 { "●" }
            else if session.user_count > 10 { "◐" }
            else { "○" };

        let time_str = session.start_time.format("%H:%M").to_string();

        // Grade from session analysis
        let grade = if let Some(analysis) = store.analyze_session(&session.session_id) {
            grade_letter(&analysis)
        } else {
            "-"
        };

        let grade_color = match grade {
            "A" => Color::Rgb(120, 190, 120),
            "B" => ACCENT,
            "C" => YELLOW,
            _ => RED,
        };

        let name_w = (w as usize).saturating_sub(55).max(8);
        let topic = truncate(&session.first_message, name_w);

        let (fg_main, fg_sub) = if is_selected {
            (FG, ACCENT)
        } else {
            (FG_MUTED, FG_FAINT)
        };

        let cursor_char = if is_selected { ">" } else { " " };

        lines.push(Line::from(vec![
            Span::styled(format!("  {}{} {} ", cursor_char, weight, time_str),
                Style::default().fg(fg_sub)),
            Span::styled(format!("{:<width$}", topic, width = name_w),
                Style::default().fg(fg_main)),
            Span::styled(format!("{:>4}m", session.user_count),
                Style::default().fg(fg_sub)),
            Span::styled(format!("{:>6}", dur_str),
                Style::default().fg(fg_sub)),
            Span::styled(format!("{:>8}", pricing::format_cost(cost)),
                Style::default().fg(if is_selected { ACCENT } else { FG_FAINT })),
            Span::styled(format!("  {}", grade),
                Style::default().fg(grade_color)),
        ]));

        visible_row += 1;
        row_idx += 1;
    }

    frame.render_widget(Paragraph::new(lines), chunks[2]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[3]);

    // Help
    let help = Line::from(vec![
        Span::styled("   \u{2191}\u{2193}", Style::default().fg(ACCENT)),
        Span::styled(" navigate  ", Style::default().fg(FG_MUTED)),
        Span::styled("enter", Style::default().fg(ACCENT)),
        Span::styled(" detail  ", Style::default().fg(FG_MUTED)),
        Span::styled("esc", Style::default().fg(ACCENT)),
        Span::styled(" back   ", Style::default().fg(FG_MUTED)),
        Span::styled("q", Style::default().fg(ACCENT)),
        Span::styled(" quit", Style::default().fg(FG_MUTED)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[4]);
}

fn render_detail(frame: &mut ratatui::Frame, store: &Store, _config: &Config, state: &mut SessionsState) {
    let area = frame.area();
    let w = area.width;
    let detail = match &state.detail {
        Some(d) => d,
        None => return,
    };

    // Find matching session meta
    let sessions = store.sessions_by_time();
    let meta = sessions.iter().find(|s| s.session_id == detail.session_id);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),   // header
            Constraint::Length(3),   // stats
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // message timeline
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help
        ])
        .split(area);

    // Header
    if let Some(meta) = meta {
        let cost = store.session_cost(&meta.session_id);
        let dur = meta.duration_minutes();
        let dur_str = if dur >= 60 {
            format!("{}h{:02}m", dur / 60, dur % 60)
        } else {
            format!("{}m", dur.max(1))
        };
        let date_str = meta.start_time.format("%Y-%m-%d %H:%M").to_string();

        let header = vec![
            Line::from(vec![
                Span::styled(format!("   {}", date_str), Style::default().fg(ACCENT).bold()),
                Span::styled(format!("  {}  ", meta.project), Style::default().fg(FG)),
                Span::styled(format!("{}m  {}  {}", meta.user_count, dur_str, pricing::format_cost(cost)),
                    Style::default().fg(FG_MUTED)),
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        // Stats: context trajectory + tool usage
        let analysis = store.analyze_session(&meta.session_id);
        let mut stat_lines: Vec<Line> = Vec::new();

        if let Some(a) = &analysis {
            let grade = grade_letter(a);
            let grade_color = match grade {
                "A" => Color::Rgb(120, 190, 120),
                "B" => ACCENT,
                "C" => YELLOW,
                _ => RED,
            };
            stat_lines.push(Line::from(vec![
                Span::styled("   context  ", Style::default().fg(FG_MUTED)),
                Span::styled(compact(a.context_initial), Style::default().fg(FG_FAINT)),
                Span::styled(" \u{2192} ", Style::default().fg(FG_FAINT)),
                Span::styled(compact(a.context_current), Style::default().fg(FG)),
                Span::styled(format!("  ({:.1}x)", a.context_growth), Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {} compactions", a.compaction_count), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  cache {:.0}%", a.cache_hit_rate * 100.0), Style::default().fg(FG_FAINT)),
                Span::styled(format!("  Grade {}", grade), Style::default().fg(grade_color).bold()),
            ]));
        }

        // Tool usage summary
        let top_tools: Vec<String> = meta.tools_used.iter()
            .take(6)
            .map(|t| {
                let count = meta.tool_counts.get(t).unwrap_or(&0);
                format!("{}({})", t, count)
            })
            .collect();
        if !top_tools.is_empty() {
            stat_lines.push(Line::from(vec![
                Span::styled("   tools    ", Style::default().fg(FG_MUTED)),
                Span::styled(top_tools.join("  "), Style::default().fg(FG_FAINT)),
            ]));
        }

        frame.render_widget(Paragraph::new(stat_lines), chunks[1]);
    }

    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // Message timeline (user messages only for clean readability)
    let max_rows = chunks[3].height as usize;
    let user_messages: Vec<&ConversationMessage> = detail.messages.iter()
        .filter(|m| m.role == "user")
        .collect();

    // Clamp scroll
    let max_scroll = user_messages.len().saturating_sub(max_rows);
    if state.detail_scroll > max_scroll {
        state.detail_scroll = max_scroll;
    }

    let mut lines: Vec<Line> = Vec::new();
    for msg in user_messages.iter().skip(state.detail_scroll).take(max_rows) {
        let time_str = msg.timestamp.format("%H:%M").to_string();
        let content_w = (w as usize).saturating_sub(14).max(10);
        let content = truncate(&msg.content, content_w);

        lines.push(Line::from(vec![
            Span::styled(format!("   {} ", time_str), Style::default().fg(FG_FAINT)),
            Span::styled("\u{25b8} ", Style::default().fg(ACCENT_DIM)),
            Span::styled(content, Style::default().fg(FG)),
        ]));
    }

    // Scroll indicator
    if user_messages.len() > max_rows {
        let pct = if max_scroll > 0 {
            state.detail_scroll as f64 / max_scroll as f64 * 100.0
        } else { 0.0 };
        if let Some(last) = lines.last_mut() {
            *last = Line::from(vec![
                Span::styled(
                    format!("{}[{:.0}%]", " ".repeat((w as usize).saturating_sub(8)), pct),
                    Style::default().fg(FG_FAINT),
                ),
            ]);
        }
    }

    frame.render_widget(Paragraph::new(lines), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);

    // Help
    let msg_count = user_messages.len();
    let help = Line::from(vec![
        Span::styled("   \u{2191}\u{2193}", Style::default().fg(ACCENT)),
        Span::styled(" scroll  ", Style::default().fg(FG_MUTED)),
        Span::styled("esc", Style::default().fg(ACCENT)),
        Span::styled(" back   ", Style::default().fg(FG_MUTED)),
        Span::styled(format!("{} messages", msg_count), Style::default().fg(FG_FAINT)),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[5]);
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

fn month_abbrev(month: u32) -> &'static str {
    match month {
        1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR",
        5 => "MAY", 6 => "JUN", 7 => "JUL", 8 => "AUG",
        9 => "SEP", 10 => "OCT", 11 => "NOV", 12 => "DEC",
        _ => "???",
    }
}
