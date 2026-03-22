pub mod dashboard;
pub mod help;
pub mod history;
pub mod sessions;
pub mod widgets;
pub mod cursor_view;

use crate::config::Config;
use crate::parser;
use crate::store::Store;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::sync::mpsc;
use std::time::Instant;

#[derive(PartialEq)]
pub enum View {
    Overview,
    ClaudeCode,
    Cursor,
    History,
}

pub struct App {
    pub store: Store,
    pub config: Config,
    pub view: View,
    pub should_quit: bool,
    pub show_help: bool,
    pub dashboard_state: dashboard::DashboardState,
    pub sessions_state: sessions::SessionsState,
    pub cursor_state: cursor_view::CursorViewState,
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
        Self {
            store,
            config,
            view: View::Overview,
            should_quit: false,
            show_help: false,
            dashboard_state: dashboard::DashboardState::default(),
            sessions_state: sessions::SessionsState::default(),
            cursor_state: cursor_view::CursorViewState::default(),
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
            View::Overview => {
                match code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Char('h') => {
                        self.scroll = 0; self.view = View::History;
                    }
                    KeyCode::Char('d') => {
                        self.sessions_state = sessions::SessionsState::default();
                        self.view = View::ClaudeCode;
                    }
                    KeyCode::Char('c') => {
                        self.cursor_state = cursor_view::CursorViewState::default();
                        self.view = View::Cursor;
                    }
                    KeyCode::Char('o') => { /* already on overview */ }
                    KeyCode::Char('?') => self.show_help = true,
                    _ => {}
                }
            }
            View::ClaudeCode => {
                // Search mode input handling
                if self.sessions_state.search_active {
                    match code {
                        KeyCode::Esc => {
                            self.sessions_state.search_active = false;
                            self.sessions_state.search_query.clear();
                        }
                        KeyCode::Enter => {
                            self.sessions_state.search_active = false;
                        }
                        KeyCode::Backspace => {
                            self.sessions_state.search_query.pop();
                        }
                        KeyCode::Char(c) => {
                            self.sessions_state.search_query.push(c);
                            self.sessions_state.cursor = 0;
                            self.sessions_state.scroll = 0;
                        }
                        _ => {}
                    }
                    return;
                }
                match code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Up | KeyCode::Char('k') => self.sessions_state.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max = self.store.sessions_by_source(crate::parser::Source::ClaudeCode).len();
                        self.sessions_state.move_down(max);
                    }
                    KeyCode::Enter => self.sessions_state.enter(&self.store),
                    KeyCode::Esc => {
                        if !self.sessions_state.back() {
                            self.view = View::Overview;
                        }
                    }
                    KeyCode::Char('s') if self.sessions_state.detail.is_none() => {
                        self.sessions_state.sort_column = self.sessions_state.sort_column.next();
                    }
                    KeyCode::Char('/') if self.sessions_state.detail.is_none() => {
                        self.sessions_state.search_active = true;
                    }
                    KeyCode::Char('h') if self.sessions_state.detail.is_none() => {
                        self.scroll = 0; self.view = View::History;
                    }
                    KeyCode::Char('c') if self.sessions_state.detail.is_none() => {
                        self.cursor_state = cursor_view::CursorViewState::default();
                        self.view = View::Cursor;
                    }
                    KeyCode::Char('o') if self.sessions_state.detail.is_none() => {
                        self.view = View::Overview;
                    }
                    KeyCode::Char('?') => self.show_help = true,
                    _ => {}
                }
            }
            View::Cursor => {
                // Search mode input handling
                if self.cursor_state.search_active {
                    match code {
                        KeyCode::Esc => {
                            self.cursor_state.search_active = false;
                            self.cursor_state.search_query.clear();
                        }
                        KeyCode::Enter => {
                            self.cursor_state.search_active = false;
                        }
                        KeyCode::Backspace => {
                            self.cursor_state.search_query.pop();
                        }
                        KeyCode::Char(c) => {
                            self.cursor_state.search_query.push(c);
                            self.cursor_state.cursor = 0;
                            self.cursor_state.scroll = 0;
                        }
                        _ => {}
                    }
                    return;
                }
                match code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Up | KeyCode::Char('k') => self.cursor_state.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max = self.store.cursor_sessions().len();
                        self.cursor_state.move_down(max);
                    }
                    KeyCode::Enter => self.cursor_state.enter(&self.store),
                    KeyCode::Esc => {
                        if !self.cursor_state.search_query.is_empty() {
                            self.cursor_state.search_query.clear();
                            self.cursor_state.cursor = 0;
                            self.cursor_state.scroll = 0;
                        } else if !self.cursor_state.back() {
                            self.view = View::Overview;
                        }
                    }
                    KeyCode::Char('s') if self.cursor_state.detail.is_none() => {
                        self.cursor_state.sort_column = self.cursor_state.sort_column.next();
                        self.cursor_state.cursor = 0;
                        self.cursor_state.scroll = 0;
                    }
                    KeyCode::Char('/') if self.cursor_state.detail.is_none() => {
                        self.cursor_state.search_active = true;
                        self.cursor_state.search_query.clear();
                    }
                    KeyCode::Char('d') if self.cursor_state.detail.is_none() => {
                        self.sessions_state = sessions::SessionsState::default();
                        self.view = View::ClaudeCode;
                    }
                    KeyCode::Char('h') if self.cursor_state.detail.is_none() => {
                        self.scroll = 0; self.view = View::History;
                    }
                    KeyCode::Char('o') if self.cursor_state.detail.is_none() => {
                        self.view = View::Overview;
                    }
                    KeyCode::Char('?') => self.show_help = true,
                    _ => {}
                }
            }
            View::History => {
                match code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.scroll = self.scroll.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.scroll += 1;
                    }
                    KeyCode::Esc => { self.scroll = 0; self.view = View::Overview; }
                    KeyCode::Char('d') => {
                        self.sessions_state = sessions::SessionsState::default();
                        self.view = View::ClaudeCode;
                    }
                    KeyCode::Char('c') => {
                        self.cursor_state = cursor_view::CursorViewState::default();
                        self.view = View::Cursor;
                    }
                    KeyCode::Char('o') => {
                        self.scroll = 0;
                        self.view = View::Overview;
                    }
                    KeyCode::Char('?') => self.show_help = true,
                    _ => {}
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        match self.view {
            View::Overview => dashboard::render(frame, &self.store, &self.config, &mut self.dashboard_state, &self.live_sessions),
            View::History => history::render(frame, &self.store, &self.config, self.scroll),
            View::ClaudeCode => sessions::render(frame, &self.store, &self.config, &mut self.sessions_state, &self.live_sessions),
            View::Cursor => cursor_view::render(frame, &self.store, &self.config, &mut self.cursor_state, &self.live_sessions),
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
