pub mod browser;
pub mod dashboard;
pub mod help;
pub mod stats;
pub mod widgets;

use crate::config::Config;
use crate::parser;
use crate::store::Store;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::sync::mpsc;
use std::time::Instant;

#[derive(PartialEq)]
pub enum View {
    Browser,
    Stats,
}

pub struct App {
    pub store: Store,
    pub config: Config,
    pub view: View,
    pub should_quit: bool,
    pub show_help: bool,
    pub browser_state: browser::BrowserState,
    pub detail_state: dashboard::DashboardState,
    pub scroll: usize,
    watcher_rx: Option<mpsc::Receiver<Vec<String>>>,
    last_cursor_refresh: Instant,
    pub live_sessions: std::collections::HashMap<String, bool>,
    last_liveness_check: Instant,
}

impl App {
    pub fn new(store: Store, config: Config) -> Self {
        let watcher_rx = parser::watcher::watch(&config.data_dir()).ok();
        let sessions_dir = dirs::home_dir().unwrap_or_default().join(".claude/sessions");
        let live_sessions = crate::parser::liveness::check_liveness(&sessions_dir);

        let (initial_view, initial_filter) = match config.default_view.as_str() {
            "claude_code" => (View::Browser, Some(browser::SourceFilter::ClaudeCode)),
            "cursor" => (View::Browser, Some(browser::SourceFilter::Cursor)),
            "stats" | "history" => (View::Stats, None),
            _ => (View::Browser, None),
        };
        let mut browser_state = browser::BrowserState::default();
        if let Some(filter) = initial_filter {
            browser_state.source_filter = filter;
        }
        Self {
            store,
            config,
            view: initial_view,
            should_quit: false,
            show_help: false,
            browser_state,
            detail_state: dashboard::DashboardState::default(),
            scroll: 0,
            watcher_rx,
            last_cursor_refresh: Instant::now(),
            live_sessions,
            last_liveness_check: Instant::now(),
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> anyhow::Result<()> {
        let tick_rate = self.config.refresh_interval_duration();
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            self.check_watcher();
            self.check_liveness();
            self.check_cursor_refresh();

            if event::poll(tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code);
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode) {
        // Help overlay takes priority
        if self.show_help {
            match code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
            return;
        }

        match self.view {
            View::Browser => {
                // Detail overlay: Esc dismisses it
                if self.detail_state.detail.is_some() {
                    match code {
                        KeyCode::Esc => { self.detail_state.back(); }
                        KeyCode::Char('q') => self.should_quit = true,
                        _ => {}
                    }
                    return;
                }

                match code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Char('?') => self.show_help = true,
                    KeyCode::Char('s') if self.browser_state.is_at_root() && !self.browser_state.search_active => {
                        self.scroll = 0; self.view = View::Stats;
                    }
                    _ => {
                        self.browser_state.handle_key(code, &self.store);
                        // Drain pending detail
                        if let Some(sid) = self.browser_state.pending_detail_session_id.take() {
                            if let Some(timeline) = self.store.session_timeline(&sid) {
                                self.detail_state.detail = Some(dashboard::SessionDetailView {
                                    session_id: sid,
                                    timeline,
                                });
                            }
                        }
                    }
                }
            }
            View::Stats => {
                match code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Char('?') => self.show_help = true,
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.scroll = self.scroll.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.scroll += 1;
                    }
                    KeyCode::Esc | KeyCode::Char('b') => {
                        self.scroll = 0; self.view = View::Browser;
                    }
                    KeyCode::Char('d') => {
                        self.browser_state = browser::BrowserState::default();
                        self.browser_state.source_filter = browser::SourceFilter::ClaudeCode;
                        self.scroll = 0; self.view = View::Browser;
                    }
                    KeyCode::Char('c') => {
                        self.browser_state = browser::BrowserState::default();
                        self.browser_state.source_filter = browser::SourceFilter::Cursor;
                        self.scroll = 0; self.view = View::Browser;
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        match self.view {
            View::Browser => {
                if self.detail_state.detail.is_some() {
                    dashboard::render_detail(frame, &self.store, &self.config, &mut self.detail_state, &self.live_sessions);
                } else {
                    browser::render(frame, &self.store, &self.config, &mut self.browser_state, &self.live_sessions);
                }
            }
            View::Stats => stats::render(frame, &self.store, &self.config, self.scroll),
        }

        if self.show_help {
            help::render_help_overlay(frame);
        }
    }

    fn check_watcher(&mut self) {
        if let Some(rx) = &self.watcher_rx {
            while let Ok(paths) = rx.try_recv() {
                for path in paths {
                    if let Ok(records) = parser::parse_file(&path) {
                        for r in records {
                            self.store.add(r);
                        }
                    }
                }
            }
        }
    }

    fn check_liveness(&mut self) {
        let interval = self.config.live_check_interval_duration();
        if self.last_liveness_check.elapsed() < interval { return; }
        self.last_liveness_check = Instant::now();
        let sessions_dir = dirs::home_dir().unwrap_or_default().join(".claude/sessions");
        self.live_sessions = crate::parser::liveness::check_liveness(&sessions_dir);
    }

    fn check_cursor_refresh(&mut self) {
        if self.last_cursor_refresh.elapsed() < std::time::Duration::from_secs(30) {
            return;
        }
        self.last_cursor_refresh = Instant::now();

        if let Some(cursor_path) = self.config.cursor_db_path() {
            if let Some(path_str) = cursor_path.to_str() {
                if let Ok((records, metas)) = parser::cursor::parse_cursor_db(path_str) {
                    let existing: std::collections::HashSet<String> = self.store.cursor_sessions()
                        .iter()
                        .map(|s| s.session_id.clone())
                        .collect();

                    for r in records {
                        if !self.config.is_excluded(&r.project) {
                            self.store.add(r);
                        }
                    }
                    for m in metas {
                        if m.user_count > 0
                            && !self.config.is_excluded(&m.project)
                            && !existing.contains(&m.session_id)
                        {
                            self.store.add_session_meta(m);
                        }
                    }
                }
            }
        }
    }
}
