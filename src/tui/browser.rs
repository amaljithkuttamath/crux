use crate::config::Config;
use crate::parser::conversation::{self, ConversationMessage};
use crate::parser::Source;
use crate::pricing;
use crate::store::Store;
use super::widgets::*;
use chrono::Utc;
use ratatui::prelude::*;
use ratatui::widgets::*;

// ═══════════════════════════════════════════════════════════════
//  Three-panel session browser. Arrow-key driven.
//  Left: project tree  |  Center: session list  |  Right: stats
//  Stats sidebar updates live as cursor moves.
// ═══════════════════════════════════════════════════════════════

/// Which panel has focus
#[derive(Default, Clone, Copy, PartialEq)]
pub enum Panel {
    #[default]
    Projects,
    Sessions,
    Conversation,
}

/// Source filter
#[derive(Default, Clone, Copy, PartialEq)]
pub enum SourceFilter {
    #[default]
    All,
    ClaudeCode,
    Cursor,
}

impl SourceFilter {
    pub fn next(&self) -> Self {
        match self {
            Self::All => Self::ClaudeCode,
            Self::ClaudeCode => Self::Cursor,
            Self::Cursor => Self::All,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::ClaudeCode => "CC",
            Self::Cursor => "Cursor",
        }
    }
    pub fn matches(&self, source: Source) -> bool {
        match self {
            Self::All => true,
            Self::ClaudeCode => source == Source::ClaudeCode,
            Self::Cursor => source == Source::Cursor,
        }
    }
}

#[derive(Default)]
pub struct BrowserState {
    pub panel: Panel,
    pub source_filter: SourceFilter,

    // Project panel
    pub project_cursor: usize,
    pub project_scroll: usize,

    // Session panel
    pub session_cursor: usize,
    pub session_scroll: usize,

    // Conversation panel
    pub conv_scroll: usize,
    pub conv_messages: Option<Vec<ConversationMessage>>,
    pub conv_session_id: Option<String>,

    // Search
    pub search_active: bool,
    pub search_query: String,

    // Cached for cursor bounds
    pub cached_projects: Vec<String>,
    pub cached_session_ids: Vec<String>,

    // Detail drill-in: set by Enter, drained by mod.rs
    pub pending_detail_session_id: Option<String>,
}

impl BrowserState {
    pub fn handle_key(&mut self, code: crossterm::event::KeyCode, store: &Store) {
        use crossterm::event::KeyCode;

        // Search mode
        if self.search_active {
            match code {
                KeyCode::Esc => {
                    self.search_active = false;
                    self.search_query.clear();
                }
                KeyCode::Enter => { self.search_active = false; }
                KeyCode::Backspace => { self.search_query.pop(); self.session_cursor = 0; self.session_scroll = 0; }
                KeyCode::Char(c) => { self.search_query.push(c); self.session_cursor = 0; self.session_scroll = 0; }
                _ => {}
            }
            return;
        }

        match code {
            // Navigation
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            KeyCode::Right | KeyCode::Char('l') => self.drill_in(store),
            KeyCode::Left | KeyCode::Char('h') if self.panel != Panel::Projects => self.drill_out(),
            KeyCode::Esc => self.drill_out(),

            // Enter: session detail (full-screen analysis view)
            KeyCode::Enter if self.panel == Panel::Sessions => {
                if let Some(sid) = self.cached_session_ids.get(self.session_cursor) {
                    self.pending_detail_session_id = Some(sid.clone());
                }
            }
            KeyCode::Enter if self.panel == Panel::Projects => {
                self.panel = Panel::Sessions;
                self.session_cursor = 0;
                self.session_scroll = 0;
            }

            // Filters
            KeyCode::Char('f') => {
                self.source_filter = self.source_filter.next();
                self.project_cursor = 0;
                self.session_cursor = 0;
                self.session_scroll = 0;
            }
            KeyCode::Char('d') if self.panel == Panel::Projects => {
                self.source_filter = SourceFilter::ClaudeCode;
                self.project_cursor = 0;
                self.session_cursor = 0;
                self.session_scroll = 0;
            }
            KeyCode::Char('c') if self.panel == Panel::Projects => {
                self.source_filter = SourceFilter::Cursor;
                self.project_cursor = 0;
                self.session_cursor = 0;
                self.session_scroll = 0;
            }

            // Search (session panel only)
            KeyCode::Char('/') if self.panel == Panel::Sessions || self.panel == Panel::Projects => {
                self.search_active = true;
                self.search_query.clear();
            }

            _ => {}
        }
    }

    fn move_up(&mut self) {
        match self.panel {
            Panel::Projects => {
                if self.project_cursor > 0 {
                    self.project_cursor -= 1;
                    self.session_cursor = 0;
                    self.session_scroll = 0;
                    self.clear_conversation();
                }
            }
            Panel::Sessions => {
                if self.session_cursor > 0 {
                    self.session_cursor -= 1;
                    self.clear_conversation();
                }
            }
            Panel::Conversation => {
                self.conv_scroll = self.conv_scroll.saturating_sub(1);
            }
        }
    }

    fn move_down(&mut self) {
        match self.panel {
            Panel::Projects => {
                if self.project_cursor + 1 < self.cached_projects.len() {
                    self.project_cursor += 1;
                    self.session_cursor = 0;
                    self.session_scroll = 0;
                    self.clear_conversation();
                }
            }
            Panel::Sessions => {
                if self.session_cursor + 1 < self.cached_session_ids.len() {
                    self.session_cursor += 1;
                    self.clear_conversation();
                }
            }
            Panel::Conversation => {
                self.conv_scroll += 1;
            }
        }
    }

    fn drill_in(&mut self, store: &Store) {
        match self.panel {
            Panel::Projects => {
                self.panel = Panel::Sessions;
                self.session_cursor = 0;
                self.session_scroll = 0;
            }
            Panel::Sessions => {
                // Load conversation
                if let Some(sid) = self.cached_session_ids.get(self.session_cursor) {
                    if let Some(meta) = store.session_meta(sid) {
                        if let Ok(msgs) = conversation::parse_conversation(&meta.file_path) {
                            self.conv_messages = Some(msgs);
                            self.conv_session_id = Some(sid.clone());
                            self.conv_scroll = 0;
                            self.panel = Panel::Conversation;
                        }
                    }
                }
            }
            Panel::Conversation => {} // nowhere deeper to go
        }
    }

    fn drill_out(&mut self) {
        match self.panel {
            Panel::Conversation => {
                self.panel = Panel::Sessions;
                self.clear_conversation();
            }
            Panel::Sessions => {
                self.panel = Panel::Projects;
            }
            Panel::Projects => {} // handled by mod.rs (go back to Overview)
        }
    }

    fn clear_conversation(&mut self) {
        self.conv_messages = None;
        self.conv_session_id = None;
        self.conv_scroll = 0;
    }

    pub fn is_at_root(&self) -> bool {
        self.panel == Panel::Projects
    }
}

// ═══════════════════════════════════════════════════════════════
//  Render
// ═══════════════════════════════════════════════════════════════

pub fn render(
    frame: &mut ratatui::Frame,
    store: &Store,
    config: &Config,
    state: &mut BrowserState,
    live_sessions: &std::collections::HashMap<String, bool>,
) {
    let area = frame.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // today cockpit (always visible)
            Constraint::Min(4),    // three panels
            Constraint::Length(1), // help
        ])
        .split(area);

    // Today cockpit header
    render_today_header(frame, store, config, state, outer[0], live_sessions);

    // Three panels: projects (20%) | center (50%) | sidebar (30%)
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(50),
            Constraint::Percentage(30),
        ])
        .split(outer[1]);

    render_projects(frame, store, state, panels[0]);

    if state.panel == Panel::Conversation {
        render_conversation(frame, state, panels[1]);
    } else {
        render_sessions(frame, store, config, state, panels[1], live_sessions);
    }

    render_sidebar(frame, store, config, state, panels[2], live_sessions);

    // Help bar: verbose since we only have 2 views
    let filter_hint = match state.source_filter {
        SourceFilter::All => "f:filter",
        SourceFilter::ClaudeCode => "f:CC",
        SourceFilter::Cursor => "f:Cursor",
    };
    let help_items: Vec<(&str, &str)> = match state.panel {
        Panel::Projects => vec![
            ("\u{2191}\u{2193}", "select project"),
            ("\u{2192}/enter", "open sessions"),
            ("d", "cc only"),
            ("c", "cursor only"),
            (filter_hint, ""),
            ("/", "search"),
            ("s", "stats"),
            ("?", "help"),
        ],
        Panel::Sessions => vec![
            ("\u{2191}\u{2193}", "select session"),
            ("enter", "detail view"),
            ("\u{2192}", "conversation"),
            ("\u{2190}", "back"),
            (filter_hint, ""),
            ("/", "search"),
            ("s", "stats"),
        ],
        Panel::Conversation => vec![
            ("\u{2191}\u{2193}", "scroll messages"),
            ("\u{2190}", "back to sessions"),
            ("s", "stats"),
            ("q", "quit"),
        ],
    };
    let help = help_bar(&help_items);
    frame.render_widget(Paragraph::new(help), outer[2]);
}

// ═══════════════════════════════════════════════════════════════
//  Today cockpit: always-visible 3-line header
//  Line 1: TODAY cost | vs avg | $/hr | 7d spark | streak
//  Line 2: CC + Cursor source split with active counts
//  Line 3: source-specific detail when filtered, divider when All
// ═══════════════════════════════════════════════════════════════

fn render_today_header(
    frame: &mut ratatui::Frame,
    store: &Store,
    config: &crate::config::Config,
    state: &BrowserState,
    area: Rect,
    live_sessions: &std::collections::HashMap<String, bool>,
) {
    let w = area.width;
    let today = store.today();
    let week = store.this_week();

    // Burn rate
    let hours_elapsed = {
        let now = Utc::now();
        use chrono::Timelike;
        (now.hour() as f64 + now.minute() as f64 / 60.0).max(0.1)
    };
    let burn_rate = today.cost / hours_elapsed;

    let spark_data = store.daily_costs(7);
    let spark_str = spark(&spark_data);
    let streak = store.streak_days();
    let rolling_avg = store.rolling_avg_daily_cost(7);

    // Line 1: TODAY ticker
    let mut t1: Vec<Span> = vec![
        Span::styled("   TODAY", Style::default().fg(ACCENT).bold()),
        Span::styled(format!("  {}", pricing::format_cost(today.cost)), Style::default().fg(ACCENT).bold()),
    ];
    if rolling_avg > 0.0 {
        let diff_pct = ((today.cost - rolling_avg) / rolling_avg * 100.0) as i64;
        let dc = if diff_pct > 50 { RED } else if diff_pct < -20 { GREEN } else { FG_FAINT };
        t1.push(Span::styled(format!("  vs avg {} ", pricing::format_cost(rolling_avg)), Style::default().fg(FG_FAINT)));
        t1.push(Span::styled(format!("{:+}%", diff_pct.clamp(-200, 200)), Style::default().fg(dc)));
    }
    t1.push(Span::styled(format!("  {}/hr", pricing::format_cost(burn_rate)), Style::default().fg(FG_MUTED)));

    // Budget bar inline if configured
    if let Some(budget) = config.budget_daily {
        let pct = today.cost / budget * 100.0;
        let bc = if pct > 90.0 { RED } else if pct > 70.0 { YELLOW } else { GREEN };
        t1.push(Span::styled(format!("  budget {:.0}%", pct), Style::default().fg(bc)));
    } else if let Some(budget) = config.budget_weekly {
        let pct = week.cost / budget * 100.0;
        let bc = if pct > 90.0 { RED } else if pct > 70.0 { YELLOW } else { GREEN };
        t1.push(Span::styled(format!("  wk budget {:.0}%", pct), Style::default().fg(bc)));
    }

    // Right-align spark + streak
    let left_len: usize = t1.iter().map(|s| s.content.len()).sum();
    let right_content = format!("{}  streak {}d", spark_str, streak);
    let pad = (w as usize).saturating_sub(left_len + right_content.chars().count() + 4).max(1);
    t1.push(Span::styled(" ".repeat(pad), Style::default()));
    t1.push(Span::styled(spark_str.clone(), Style::default().fg(ACCENT)));
    t1.push(Span::styled(format!("  streak {}d", streak), Style::default().fg(ACCENT)));

    // Line 2: source split
    let cc_agg = store.today_by_source(Source::ClaudeCode);
    let cu_agg = store.today_by_source(Source::Cursor);
    let all_sessions = store.sessions_by_time();
    let cc_active = all_sessions.iter()
        .filter(|s| s.source == Source::ClaudeCode && !s.is_subagent)
        .filter(|s| live_sessions.get(&s.session_id).copied().unwrap_or(false))
        .count();
    let cu_active = all_sessions.iter()
        .filter(|s| s.source == Source::Cursor && !s.is_subagent)
        .filter(|s| live_sessions.get(&s.session_id).copied().unwrap_or(false))
        .count();
    let cc_sess_today = store.today_sessions_by_source(Source::ClaudeCode).into_iter()
        .filter(|s| !s.is_subagent).count();
    let cu_sess_today = store.today_sessions_by_source(Source::Cursor).into_iter()
        .filter(|s| !s.is_subagent).count();

    let mut t2: Vec<Span> = vec![Span::styled("   ", Style::default())];
    if cc_agg.cost > 0.0 || cc_sess_today > 0 {
        t2.push(Span::styled("\u{25cf}", Style::default().fg(ACCENT2)));
        t2.push(Span::styled(format!(" CC {}  {} sess", pricing::format_cost(cc_agg.cost), cc_sess_today), Style::default().fg(FG_MUTED)));
        if cc_active > 0 {
            t2.push(Span::styled(format!("  {} active", cc_active), Style::default().fg(GREEN)));
        }
    }
    if cu_agg.cost > 0.0 || cu_sess_today > 0 {
        if cc_agg.cost > 0.0 || cc_sess_today > 0 { t2.push(Span::styled("     ", Style::default())); }
        t2.push(Span::styled("\u{25cf}", Style::default().fg(BLUE)));
        t2.push(Span::styled(format!(" Cursor {}  {} sess", pricing::format_cost(cu_agg.cost), cu_sess_today), Style::default().fg(FG_MUTED)));
        if cu_active > 0 {
            t2.push(Span::styled(format!("  {} active", cu_active), Style::default().fg(GREEN)));
        }
    }
    // Filter badge on the right
    if state.source_filter != SourceFilter::All {
        let filter_label = match state.source_filter {
            SourceFilter::ClaudeCode => "CC only",
            SourceFilter::Cursor => "Cursor only",
            SourceFilter::All => "",
        };
        let t2_len: usize = t2.iter().map(|s| s.content.len()).sum();
        let fpad = (w as usize).saturating_sub(t2_len + filter_label.len() + 6).max(1);
        t2.push(Span::styled(" ".repeat(fpad), Style::default()));
        t2.push(Span::styled(format!("[{}]", filter_label), Style::default().fg(ACCENT)));
    }

    // Line 3: divider
    let t3 = divider(w);

    frame.render_widget(Paragraph::new(vec![
        Line::from(t1),
        Line::from(t2),
        t3,
    ]), area);
}

fn render_projects(
    frame: &mut ratatui::Frame,
    store: &Store,
    state: &mut BrowserState,
    area: Rect,
) {
    let is_focused = state.panel == Panel::Projects;
    let border_color = if is_focused { ACCENT } else { FG_FAINT };

    // Build project list: "All" + per-project, filtered by source
    let projects = store.by_project();

    // Only show projects that have sessions matching the source filter
    let sessions = store.sessions_by_time();
    let source_projects: std::collections::HashSet<&str> = sessions.iter()
        .filter(|s| state.source_filter.matches(s.source))
        .map(|s| s.project.as_str())
        .collect();

    let mut project_names: Vec<String> = Vec::new();
    project_names.push("__all__".to_string());

    let q = state.search_query.to_lowercase();
    for p in &projects {
        if !source_projects.contains(p.name.as_str()) { continue; }
        if !q.is_empty() && !display_project_name(&p.name).to_lowercase().contains(&q) {
            continue;
        }
        project_names.push(p.name.clone());
    }

    state.cached_projects = project_names.clone();
    let cursor = state.project_cursor.min(project_names.len().saturating_sub(1));
    state.project_cursor = cursor;

    let max_rows = area.height.saturating_sub(2) as usize;
    // Scroll
    if cursor >= state.project_scroll + max_rows {
        state.project_scroll = cursor.saturating_sub(max_rows - 1);
    }
    if cursor < state.project_scroll {
        state.project_scroll = cursor;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Header
    let filter_label = match state.source_filter {
        SourceFilter::All => "All sources",
        SourceFilter::ClaudeCode => "Claude Code",
        SourceFilter::Cursor => "Cursor",
    };
    lines.push(Line::from(vec![
        Span::styled(format!(" {}", filter_label), Style::default().fg(if is_focused { ACCENT } else { FG_FAINT })),
    ]));
    lines.push(Line::from(Span::styled(
        format!(" {}", "\u{2500}".repeat(area.width.saturating_sub(2) as usize)),
        Style::default().fg(border_color),
    )));

    for (i, name) in project_names.iter().skip(state.project_scroll).take(max_rows).enumerate() {
        let idx = i + state.project_scroll;
        let is_selected = idx == cursor;
        let prefix = if is_selected && is_focused { "\u{25b8} " } else { "  " };

        if name == "__all__" {
            let all = store.all_time();
            let fg = if is_selected && is_focused { FG } else { FG_MUTED };
            lines.push(Line::from(vec![
                Span::styled(format!(" {}", prefix), Style::default().fg(ACCENT)),
                Span::styled("All", Style::default().fg(fg).bold()),
                Span::styled(format!("  {}", all.session_count), Style::default().fg(FG_FAINT)),
            ]));
        } else {
            let display = display_project_name(name);
            let proj_data = projects.iter().find(|p| p.name == *name);
            let sess_count = proj_data.map(|p| p.session_count).unwrap_or(0);
            let fg = if is_selected && is_focused { FG } else { FG_MUTED };
            lines.push(Line::from(vec![
                Span::styled(format!(" {}", prefix), Style::default().fg(ACCENT)),
                Span::styled(
                    truncate(&display, area.width.saturating_sub(8) as usize),
                    Style::default().fg(fg),
                ),
                Span::styled(format!(" {}", sess_count), Style::default().fg(FG_FAINT)),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_sessions(
    frame: &mut ratatui::Frame,
    store: &Store,
    _config: &Config,
    state: &mut BrowserState,
    area: Rect,
    live_sessions: &std::collections::HashMap<String, bool>,
) {
    let is_focused = state.panel == Panel::Sessions;
    let border_color = if is_focused { ACCENT } else { FG_FAINT };

    // Get selected project
    let selected_project = state.cached_projects.get(state.project_cursor).cloned();
    let filter_all = selected_project.as_deref() == Some("__all__");

    // Get sessions filtered by project and source
    let sessions = store.sessions_by_time();
    let filtered: Vec<_> = sessions.iter()
        .filter(|s| !s.is_subagent)
        .filter(|s| state.source_filter.matches(s.source))
        .filter(|s| {
            if filter_all { true }
            else if let Some(ref proj) = selected_project { s.project == *proj }
            else { true }
        })
        .filter(|s| {
            if state.search_query.is_empty() { return true; }
            let q = state.search_query.to_lowercase();
            s.first_message.to_lowercase().contains(&q)
                || display_project_name(&s.project).to_lowercase().contains(&q)
        })
        .collect();

    state.cached_session_ids = filtered.iter().map(|s| s.session_id.clone()).collect();
    let cursor = state.session_cursor.min(filtered.len().saturating_sub(1));
    state.session_cursor = cursor;

    let max_rows = area.height.saturating_sub(3) as usize;
    if cursor >= state.session_scroll + max_rows {
        state.session_scroll = cursor.saturating_sub(max_rows - 1);
    }
    if cursor < state.session_scroll {
        state.session_scroll = cursor;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Header
    if state.search_active {
        lines.push(Line::from(vec![
            Span::styled(" /", Style::default().fg(ACCENT)),
            Span::styled(state.search_query.clone(), Style::default().fg(FG)),
            Span::styled("\u{2588}", Style::default().fg(ACCENT)),
        ]));
    } else {
        let project_label = if filter_all {
            "All sessions".to_string()
        } else {
            selected_project.as_ref().map(|p| display_project_name(p)).unwrap_or_default()
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {}", project_label), Style::default().fg(if is_focused { ACCENT } else { FG_FAINT })),
            Span::styled(format!("  ({})", filtered.len()), Style::default().fg(FG_FAINT)),
        ]));
    }
    lines.push(Line::from(Span::styled(
        format!(" {}", "\u{2500}".repeat(area.width.saturating_sub(2) as usize)),
        Style::default().fg(border_color),
    )));

    // Column header
    lines.push(Line::from(vec![
        Span::styled(" DATE        TOPIC", Style::default().fg(FG_FAINT)),
    ]));

    let topic_w = (area.width as usize).saturating_sub(20);

    for (i, s) in filtered.iter().skip(state.session_scroll).take(max_rows).enumerate() {
        let idx = i + state.session_scroll;
        let is_selected = idx == cursor;
        let is_live = live_sessions.get(&s.session_id).copied().unwrap_or(false);

        let date = s.start_time.format("%m/%d %H:%M").to_string();
        let source_badge = match s.source {
            Source::ClaudeCode => ("\u{25cf}", ACCENT2),
            Source::Cursor => ("\u{25cf}", BLUE),
        };

        let fg = if is_selected && is_focused { FG } else { FG_MUTED };
        let prefix = if is_selected && is_focused { "\u{25b8}" } else { " " };

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(ACCENT)),
            Span::styled(source_badge.0, Style::default().fg(source_badge.1)),
            Span::styled(format!("{} ", date), Style::default().fg(FG_FAINT)),
            Span::styled(truncate(&s.first_message, topic_w), Style::default().fg(fg)),
            if is_live {
                Span::styled(" \u{25cf}", Style::default().fg(GREEN))
            } else {
                Span::styled("", Style::default())
            },
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_conversation(
    frame: &mut ratatui::Frame,
    state: &mut BrowserState,
    area: Rect,
) {
    let messages = match &state.conv_messages {
        Some(m) => m,
        None => {
            frame.render_widget(Paragraph::new(" No conversation loaded"), area);
            return;
        }
    };

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(" Conversation", Style::default().fg(ACCENT)),
        Span::styled(format!("  {} messages", messages.len()), Style::default().fg(FG_FAINT)),
    ]));
    lines.push(Line::from(Span::styled(
        format!(" {}", "\u{2500}".repeat(area.width.saturating_sub(2) as usize)),
        Style::default().fg(ACCENT),
    )));

    let max_rows = area.height.saturating_sub(2) as usize;
    let max_scroll = messages.len().saturating_sub(max_rows);
    if state.conv_scroll > max_scroll { state.conv_scroll = max_scroll; }

    let content_w = (area.width as usize).saturating_sub(12).max(10);

    for msg in messages.iter().skip(state.conv_scroll).take(max_rows) {
        let time_str = msg.timestamp.format("%H:%M").to_string();

        if msg.role == "user" {
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", time_str), Style::default().fg(FG_FAINT)),
                Span::styled("\u{25b8} ", Style::default().fg(ACCENT)),
                Span::styled(truncate(&msg.content, content_w), Style::default().fg(FG)),
            ]));
        } else if !msg.tool_names.is_empty() {
            let tool_str = msg.tool_names.iter().take(5)
                .map(|t| shorten_tool(t)).collect::<Vec<_>>().join(" ");
            let extra = if msg.tool_names.len() > 5 {
                format!(" +{}", msg.tool_names.len() - 5)
            } else { String::new() };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", time_str), Style::default().fg(FG_FAINT)),
                Span::styled("\u{2192} ", Style::default().fg(FG_FAINT)),
                Span::styled(format!("{}{}", tool_str, extra), Style::default().fg(FG_FAINT)),
            ]));
        } else if !msg.content.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", time_str), Style::default().fg(FG_FAINT)),
                Span::styled("\u{25c2} ", Style::default().fg(ACCENT2)),
                Span::styled(truncate(&msg.content, content_w), Style::default().fg(FG_MUTED)),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

// ═══════════════════════════════════════════════════════════════
//  Sidebar: updates based on what's selected
// ═══════════════════════════════════════════════════════════════

fn render_sidebar(
    frame: &mut ratatui::Frame,
    store: &Store,
    config: &Config,
    state: &BrowserState,
    area: Rect,
    live_sessions: &std::collections::HashMap<String, bool>,
) {
    match state.panel {
        Panel::Projects => render_sidebar_project(frame, store, state, area),
        Panel::Sessions | Panel::Conversation => {
            render_sidebar_session(frame, store, config, state, area, live_sessions);
        }
    }
}

fn render_sidebar_project(
    frame: &mut ratatui::Frame,
    store: &Store,
    state: &BrowserState,
    area: Rect,
) {
    let selected = state.cached_projects.get(state.project_cursor).cloned();
    let is_all = selected.as_deref() == Some("__all__");

    let mut lines: Vec<Line> = Vec::new();

    if is_all {
        let all = store.all_time();
        let streak = store.streak_days();

        lines.push(Line::from(vec![
            Span::styled(" All Sessions", Style::default().fg(ACCENT).bold()),
        ]));
        lines.push(Line::from(Span::styled(
            format!(" {}", "\u{2500}".repeat(area.width.saturating_sub(2) as usize)),
            Style::default().fg(FG_FAINT),
        )));
        lines.push(stat_ln("Sessions", &all.session_count.to_string()));
        lines.push(stat_ln("Cost", &pricing::format_cost(all.cost)));
        lines.push(stat_ln("Tokens", &compact(all.input_tokens + all.output_tokens)));
        lines.push(stat_ln("Streak", &format!("{} days", streak)));
        lines.push(Line::from(Span::raw("")));

        // 30d sparkline
        let daily = store.daily_costs(30);
        let sp = spark(&daily);
        lines.push(Line::from(vec![
            Span::styled(" 30d ", Style::default().fg(FG_FAINT)),
            Span::styled(sp, Style::default().fg(ACCENT)),
        ]));

        // Model breakdown
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(vec![
            Span::styled(" MODELS", Style::default().fg(FG_FAINT)),
        ]));
        let models = store.by_model();
        let total: f64 = models.iter().map(|m| m.cost).sum();
        for m in models.iter().take(4) {
            let pct = if total > 0.0 { m.cost / total * 100.0 } else { 0.0 };
            let mc = match m.name.as_str() {
                "opus" => PURPLE, "sonnet" => ACCENT, "haiku" => ACCENT2, _ => FG_MUTED,
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {:>7} ", capitalize(&m.name)), Style::default().fg(mc)),
                Span::styled(format!("{:.0}%", pct), Style::default().fg(FG_MUTED)),
            ]));
        }
    } else if let Some(ref proj_name) = selected {
        let projects = store.by_project();
        let proj = projects.iter().find(|p| p.name == *proj_name);
        let display = display_project_name(proj_name);

        lines.push(Line::from(vec![
            Span::styled(format!(" {}", truncate(&display, area.width.saturating_sub(2) as usize)),
                Style::default().fg(ACCENT).bold()),
        ]));
        lines.push(Line::from(Span::styled(
            format!(" {}", "\u{2500}".repeat(area.width.saturating_sub(2) as usize)),
            Style::default().fg(FG_FAINT),
        )));

        if let Some(p) = proj {
            lines.push(stat_ln("Sessions", &p.session_count.to_string()));
            lines.push(stat_ln("Cost", &pricing::format_cost(p.cost)));
            lines.push(stat_ln("Tokens", &compact(p.input_tokens + p.output_tokens)));
            lines.push(stat_ln("Last used", &format_ago(p.last_used)));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_sidebar_session(
    frame: &mut ratatui::Frame,
    store: &Store,
    _config: &Config,
    state: &BrowserState,
    area: Rect,
    live_sessions: &std::collections::HashMap<String, bool>,
) {
    let sid = if state.panel == Panel::Conversation {
        state.conv_session_id.clone()
    } else {
        state.cached_session_ids.get(state.session_cursor).cloned()
    };

    let sid = match sid {
        Some(s) => s,
        None => {
            frame.render_widget(Paragraph::new(" Select a session"), area);
            return;
        }
    };

    let meta = store.session_meta(&sid);
    let analysis = store.analyze_session(&sid);
    let (total_in, total_out) = store.session_tokens(&sid);
    let is_live = live_sessions.get(&sid).copied().unwrap_or(false);

    let mut lines: Vec<Line> = Vec::new();

    if let Some(meta) = meta {
        let dur = meta.duration_minutes();
        let dur_str = if dur >= 60 { format!("{}h {:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };
        let is_cursor = meta.source == Source::Cursor;

        // Header
        let source_badge = if is_cursor { ("C", BLUE) } else { ("CC", ACCENT2) };
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", source_badge.0), Style::default().fg(source_badge.1).bold()),
            Span::styled(if is_cursor { "Cursor" } else { "Claude Code" }, Style::default().fg(FG_MUTED)),
            if is_live {
                Span::styled("  \u{25cf} live", Style::default().fg(GREEN))
            } else {
                Span::styled("", Style::default())
            },
        ]));
        lines.push(Line::from(Span::styled(
            format!(" {}", "\u{2500}".repeat(area.width.saturating_sub(2) as usize)),
            Style::default().fg(FG_FAINT),
        )));

        if is_cursor {
            // ── Cursor-specific sidebar ──
            // Actual model name
            if let Some(ref model) = meta.cursor_model_name {
                if !model.is_empty() {
                    let display_model = model
                        .replace("claude-", "")
                        .replace("-high-thinking", " HT")
                        .replace("-medium-thinking", " MT")
                        .replace("-thinking", " T");
                    lines.push(Line::from(vec![
                        Span::styled(" Model  ", Style::default().fg(FG_FAINT)),
                        Span::styled(
                            truncate(&display_model, area.width.saturating_sub(9) as usize),
                            Style::default().fg(if model.contains("opus") { PURPLE }
                                else if model.contains("sonnet") { ACCENT }
                                else if model.contains("gpt") { GREEN }
                                else { FG }),
                        ),
                    ]));
                }
            }

            lines.push(stat_ln("Duration", &dur_str));
            lines.push(stat_ln("Messages", &meta.user_count.to_string()));

            // Mode + Status on one line
            let mode_label = meta.cursor_mode.as_ref().map(|m| match m {
                crate::parser::conversation::SessionMode::Agent => ("Agent", PURPLE),
                crate::parser::conversation::SessionMode::Chat => ("Chat", FG),
                crate::parser::conversation::SessionMode::Plan => ("Plan", ACCENT),
            });
            let status_info = meta.cursor_status.as_ref().map(|s| match s {
                crate::parser::conversation::SessionStatus::Completed => ("Done", GREEN),
                crate::parser::conversation::SessionStatus::Aborted => ("Aborted", RED),
                crate::parser::conversation::SessionStatus::None => ("", FG_FAINT),
            });
            if mode_label.is_some() || status_info.is_some() {
                let mut spans = vec![Span::styled(" ", Style::default())];
                if let Some((ml, mc)) = mode_label {
                    spans.push(Span::styled(ml, Style::default().fg(mc)));
                }
                if let Some((sl, sc)) = status_info {
                    if !sl.is_empty() {
                        spans.push(Span::styled(format!("  {}", sl), Style::default().fg(sc)));
                    }
                }
                if meta.is_agentic == Some(true) {
                    spans.push(Span::styled("  \u{2605}", Style::default().fg(PURPLE)));
                }
                lines.push(Line::from(spans));
            }

            // Subtitle (files edited)
            if let Some(ref sub) = meta.cursor_subtitle {
                if !sub.is_empty() {
                    lines.push(Line::from(Span::raw("")));
                    // Parse "Edited foo.rs, bar.rs" or "Read foo.rs, bar.rs"
                    let sub_w = area.width.saturating_sub(2) as usize;
                    for chunk in sub.as_bytes().chunks(sub_w) {
                        let s = String::from_utf8_lossy(chunk);
                        lines.push(Line::from(vec![
                            Span::styled(format!(" {}", s), Style::default().fg(FG_MUTED)),
                        ]));
                    }
                }
            }

            // Token counts (from bubbles)
            if total_in > 0 || total_out > 0 {
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(vec![
                    Span::styled(" TOKENS", Style::default().fg(FG_FAINT)),
                ]));
                let max_tok = total_in.max(total_out).max(1);
                let tok_bar_w = (area.width as usize).saturating_sub(14).min(10);
                let (bi, bei) = smooth_bar(total_in as f64, max_tok as f64, tok_bar_w);
                let (bo, beo) = smooth_bar(total_out as f64, max_tok as f64, tok_bar_w);
                lines.push(Line::from(vec![
                    Span::styled(" in  ", Style::default().fg(FG_FAINT)),
                    Span::styled(bi, Style::default().fg(ACCENT2)),
                    Span::styled(bei, Style::default().fg(FG_FAINT)),
                    Span::styled(format!(" {}", compact(total_in)), Style::default().fg(FG_MUTED)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(" out ", Style::default().fg(FG_FAINT)),
                    Span::styled(bo, Style::default().fg(ACCENT)),
                    Span::styled(beo, Style::default().fg(FG_FAINT)),
                    Span::styled(format!(" {}", compact(total_out)), Style::default().fg(FG_MUTED)),
                ]));
            }

            // Context usage (Cursor provides this directly)
            if let Some(ctx_pct) = meta.context_usage_pct {
                lines.push(Line::from(Span::raw("")));
                let ctx_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { GREEN };
                let bar_w = (area.width as usize).saturating_sub(12).min(12);
                let (bf, be) = smooth_bar(ctx_pct, 100.0, bar_w);
                lines.push(Line::from(vec![
                    Span::styled(" CTX  ", Style::default().fg(FG_FAINT)),
                    Span::styled(bf, Style::default().fg(ctx_color)),
                    Span::styled(be, Style::default().fg(FG_FAINT)),
                    Span::styled(format!(" {:.0}%", ctx_pct), Style::default().fg(ctx_color)),
                ]));
                if let (Some(used), Some(limit)) = (meta.context_tokens_used, meta.context_token_limit) {
                    lines.push(Line::from(vec![
                        Span::styled(format!(" {}/{}", compact(used), compact(limit)), Style::default().fg(FG_MUTED)),
                    ]));
                }
            }

            // Lines changed
            let added = meta.lines_added.unwrap_or(0);
            let removed = meta.lines_removed.unwrap_or(0);
            let files = meta.files_changed.unwrap_or(0);
            let files_added = meta.added_files.unwrap_or(0);
            let files_removed = meta.removed_files.unwrap_or(0);
            if added > 0 || removed > 0 || files > 0 || files_added > 0 || files_removed > 0 {
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(vec![
                    Span::styled(" CHANGES", Style::default().fg(FG_FAINT)),
                ]));
                if files > 0 {
                    lines.push(stat_ln("Edited", &format!("{} files", files)));
                }
                if files_added > 0 || files_removed > 0 {
                    lines.push(Line::from(vec![
                        Span::styled(format!(" +{} files", files_added), Style::default().fg(GREEN)),
                        Span::styled(format!("  -{} files", files_removed), Style::default().fg(RED)),
                    ]));
                }
                if added > 0 || removed > 0 {
                    lines.push(Line::from(vec![
                        Span::styled(format!(" +{} lines", added), Style::default().fg(GREEN)),
                        Span::styled(format!("  -{} lines", removed), Style::default().fg(RED)),
                    ]));
                }
            }

            // Todos
            if let Some(ref todos) = meta.cursor_todos {
                if !todos.is_empty() {
                    lines.push(Line::from(Span::raw("")));
                    lines.push(Line::from(vec![
                        Span::styled(" TODOS", Style::default().fg(FG_FAINT)),
                    ]));
                    for todo in todos.iter().take(5) {
                        let check = if todo.completed { "\u{2713}" } else { "\u{25cb}" };
                        let color = if todo.completed { GREEN } else { FG_MUTED };
                        lines.push(Line::from(vec![
                            Span::styled(format!(" {} ", check), Style::default().fg(color)),
                            Span::styled(
                                truncate(&todo.content, area.width.saturating_sub(5) as usize),
                                Style::default().fg(color),
                            ),
                        ]));
                    }
                }
            }
        } else {
            // ── Claude Code sidebar ──
            let cost = analysis.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
            let model = store.session_model(&sid);
            let model_short = crate::store::simplify_model(&model);
            let grade = analysis.as_ref().map(|a| a.grade_letter()).unwrap_or("-");
            let grade_color = match grade {
                "A" => GREEN, "B" => ACCENT2, "C" => ACCENT, "D" => YELLOW, _ => RED,
            };
            let mc = match model_short.as_str() {
                "opus" => PURPLE, "sonnet" => ACCENT, "haiku" => ACCENT2, _ => FG,
            };

            lines.push(Line::from(vec![
                Span::styled(format!(" Grade {} ", grade), Style::default().fg(grade_color).bold()),
                Span::styled(pricing::format_cost(cost), Style::default().fg(ACCENT)),
            ]));
            lines.push(stat_ln("Duration", &dur_str));
            lines.push(Line::from(vec![
                Span::styled(" Model  ", Style::default().fg(FG_FAINT)),
                Span::styled(capitalize(&model_short), Style::default().fg(mc)),
            ]));
            lines.push(stat_ln("Messages", &meta.user_count.to_string()));

            if let Some(ref a) = analysis {
                lines.push(Line::from(Span::raw("")));

                // Context bar
                let ceiling = meta.context_token_limit;
                let (ctx_pct, ctx_label) = if let Some(ceil) = ceiling {
                    let pct = (a.context_current as f64 / ceil as f64 * 100.0).min(100.0);
                    (pct, format!("{:.0}%", pct))
                } else {
                    (0.0, compact(a.context_current))
                };
                let ctx_color = if ctx_pct > 85.0 { RED } else if ctx_pct > 60.0 { YELLOW } else { GREEN };
                let bar_w = (area.width as usize).saturating_sub(12).min(12);
                let (bf, be) = smooth_bar(ctx_pct, 100.0, bar_w);

                lines.push(Line::from(vec![
                    Span::styled(" CTX  ", Style::default().fg(FG_FAINT)),
                    Span::styled(bf, Style::default().fg(ctx_color)),
                    Span::styled(be, Style::default().fg(FG_FAINT)),
                    Span::styled(format!(" {}", ctx_label), Style::default().fg(ctx_color)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(format!(" {} \u{2192} {}", compact(a.context_initial), compact(a.context_current)),
                        Style::default().fg(FG_MUTED)),
                    Span::styled(format!("  {:.1}x", a.context_growth), Style::default().fg(FG_FAINT)),
                ]));
                lines.push(stat_ln("Cache", &format!("{:.0}%", a.cache_hit_rate * 100.0)));

                if a.compaction_count > 0 {
                    lines.push(Line::from(vec![
                        Span::styled(" Compacts  ", Style::default().fg(FG_FAINT)),
                        Span::styled(a.compaction_count.to_string(), Style::default().fg(YELLOW)),
                    ]));
                }

                // Token bars
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(vec![
                    Span::styled(" TOKENS", Style::default().fg(FG_FAINT)),
                ]));

                let max_tok = total_in.max(total_out).max(1);
                let tok_bar_w = (area.width as usize).saturating_sub(14).min(10);
                let (bi, bei) = smooth_bar(total_in as f64, max_tok as f64, tok_bar_w);
                let (bo, beo) = smooth_bar(total_out as f64, max_tok as f64, tok_bar_w);
                lines.push(Line::from(vec![
                    Span::styled(" in  ", Style::default().fg(FG_FAINT)),
                    Span::styled(bi, Style::default().fg(ACCENT2)),
                    Span::styled(bei, Style::default().fg(FG_FAINT)),
                    Span::styled(format!(" {}", compact(total_in)), Style::default().fg(FG_MUTED)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(" out ", Style::default().fg(FG_FAINT)),
                    Span::styled(bo, Style::default().fg(ACCENT)),
                    Span::styled(beo, Style::default().fg(FG_FAINT)),
                    Span::styled(format!(" {}", compact(total_out)), Style::default().fg(FG_MUTED)),
                ]));

                // Cost breakdown
                lines.push(Line::from(Span::raw("")));
                let cb = &a.cost_breakdown;
                lines.push(Line::from(vec![
                    Span::styled(format!(" out {}  in {}", pricing::format_cost(cb.output), pricing::format_cost(cb.input)),
                        Style::default().fg(FG_MUTED)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(format!(" c-r {}  c-w {}", pricing::format_cost(cb.cache_read), pricing::format_cost(cb.cache_write)),
                        Style::default().fg(FG_MUTED)),
                ]));

                // Context sparkline
                if let Some(tl) = store.session_timeline(&sid) {
                    let ctx_vals: Vec<f64> = tl.turns.iter().map(|t| t.context_size as f64).collect();
                    if !ctx_vals.is_empty() {
                        lines.push(Line::from(Span::raw("")));
                        let sp = spark(&ctx_vals);
                        lines.push(Line::from(vec![
                            Span::styled(" ctx ", Style::default().fg(FG_FAINT)),
                            Span::styled(sp, Style::default().fg(FG_MUTED)),
                        ]));
                    }
                }
            }

            // Subagents
            let subagents = store.subagents_for(&sid);
            if !subagents.is_empty() {
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(vec![
                    Span::styled(format!(" AGENTS ({})", subagents.len()), Style::default().fg(FG_FAINT)),
                ]));
                let max_show = 6.min(subagents.len());
                for (i, (sub, sub_ana)) in subagents.iter().take(max_show).enumerate() {
                    let is_last = i == max_show - 1;
                    let tree = if is_last { "\u{2514}" } else { "\u{251c}" };
                    let sub_type = sub.agent_type.as_deref().unwrap_or("agent");
                    // Shorten type name
                    let short_type = sub_type
                        .split(':').next_back().unwrap_or(sub_type)
                        .replace("general-purpose", "general")
                        .replace("code-reviewer", "reviewer")
                        .replace("code-architect", "architect");
                    let sub_model = crate::store::simplify_model(&store.session_model(&sub.session_id));
                    let sub_cost = sub_ana.as_ref().map(|a| a.total_cost).unwrap_or(0.0);
                    let mc = match sub_model.as_str() {
                        "opus" => PURPLE, "sonnet" => ACCENT, "haiku" => ACCENT2, _ => FG_MUTED,
                    };
                    lines.push(Line::from(vec![
                        Span::styled(format!(" {} ", tree), Style::default().fg(FG_FAINT)),
                        Span::styled(
                            truncate(&short_type, 10),
                            Style::default().fg(FG_MUTED),
                        ),
                        Span::styled(format!(" {}", &sub_model[..1]), Style::default().fg(mc)),
                        Span::styled(format!(" {}", pricing::format_cost(sub_cost)), Style::default().fg(FG_FAINT)),
                    ]));
                }
                if subagents.len() > max_show {
                    lines.push(Line::from(vec![
                        Span::styled(format!("   +{} more", subagents.len() - max_show), Style::default().fg(FG_FAINT)),
                    ]));
                }
            }

            // Model mix (parent + subagents combined)
            let model_mix = store.session_model_mix(&sid);
            if model_mix.len() > 1 {
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(vec![
                    Span::styled(" MODEL MIX", Style::default().fg(FG_FAINT)),
                ]));
                let mix_bar_w = (area.width as usize).saturating_sub(18).min(8);
                for (m, _cost, pct) in model_mix.iter().take(3) {
                    let mc = match m.as_str() {
                        "opus" => PURPLE, "sonnet" => ACCENT, "haiku" => ACCENT2, _ => FG_MUTED,
                    };
                    let (bf, be) = smooth_bar(*pct, 100.0, mix_bar_w);
                    lines.push(Line::from(vec![
                        Span::styled(format!(" {:>6} ", capitalize(m)), Style::default().fg(mc)),
                        Span::styled(bf, Style::default().fg(mc)),
                        Span::styled(be, Style::default().fg(FG_FAINT)),
                        Span::styled(format!(" {:.0}%", pct), Style::default().fg(FG_MUTED)),
                    ]));
                }
            }

            // Tools
            if !meta.tools_used.is_empty() {
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(vec![
                    Span::styled(" TOOLS", Style::default().fg(FG_FAINT)),
                ]));
                let chunks: Vec<String> = meta.tools_used.iter().take(6)
                    .map(|t| {
                        let c = meta.tool_counts.get(t).unwrap_or(&0);
                        format!("{}({})", shorten_tool(t), c)
                    })
                    .collect();
                for chunk in chunks.chunks(3) {
                    lines.push(Line::from(vec![
                        Span::styled(format!(" {}", chunk.join("  ")), Style::default().fg(FG_MUTED)),
                    ]));
                }
            }
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn stat_ln(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {}  ", label), Style::default().fg(FG_FAINT)),
        Span::styled(value.to_string(), Style::default().fg(FG)),
    ])
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
