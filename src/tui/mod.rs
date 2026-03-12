pub mod dashboard;
pub mod history;
pub mod sessions;
pub mod widgets;

use crate::config::Config;
use crate::parser;
use crate::store::Store;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::sync::mpsc;

#[derive(PartialEq)]
pub enum View {
    Dashboard,
    History,
    Sessions,
}

pub struct App {
    pub store: Store,
    pub config: Config,
    pub view: View,
    pub should_quit: bool,
    pub dashboard_state: dashboard::DashboardState,
    pub sessions_state: sessions::SessionsState,
    pub scroll: usize,
    watcher_rx: Option<mpsc::Receiver<Vec<String>>>,
}

impl App {
    pub fn new(store: Store, config: Config) -> Self {
        let watcher_rx = parser::watcher::watch(&config.data_dir()).ok();
        Self {
            store,
            config,
            view: View::Dashboard,
            should_quit: false,
            dashboard_state: dashboard::DashboardState::default(),
            sessions_state: sessions::SessionsState::default(),
            scroll: 0,
            watcher_rx,
        }
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> anyhow::Result<()> {
        let tick_rate = self.config.refresh_interval_duration();
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            self.check_watcher();

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
        match self.view {
            View::Dashboard => {
                match code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.dashboard_state.move_up();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let active_count = self.store.active_sessions(24).len();
                        let project_count = self.store.by_project().len();
                        self.dashboard_state.move_down(active_count, project_count);
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        let active_count = self.store.active_sessions(24).len();
                        self.dashboard_state.switch_focus(active_count);
                    }
                    KeyCode::Enter => {
                        self.dashboard_state.enter(&self.store);
                    }
                    KeyCode::Esc => {
                        if !self.dashboard_state.back() {
                            // already at top level
                        }
                    }
                    KeyCode::Char('h') if self.dashboard_state.detail.is_none() => {
                        self.scroll = 0; self.view = View::History;
                    }
                    KeyCode::Char('s') if self.dashboard_state.detail.is_none() => {
                        self.sessions_state = sessions::SessionsState::default();
                        self.view = View::Sessions;
                    }
                    _ => {}
                }
            }
            View::Sessions => {
                match code {
                    KeyCode::Char('q') => self.should_quit = true,
                    KeyCode::Up | KeyCode::Char('k') => self.sessions_state.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max = self.store.sessions_by_time().len();
                        self.sessions_state.move_down(max);
                    }
                    KeyCode::Enter => self.sessions_state.enter(&self.store),
                    KeyCode::Esc => {
                        if !self.sessions_state.back() {
                            self.view = View::Dashboard;
                        }
                    }
                    KeyCode::Char('h') if self.sessions_state.detail.is_none() => {
                        self.scroll = 0; self.view = View::History;
                    }
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
                    KeyCode::Esc => { self.scroll = 0; self.view = View::Dashboard; }
                    KeyCode::Char('s') => {
                        self.sessions_state = sessions::SessionsState::default();
                        self.view = View::Sessions;
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        match self.view {
            View::Dashboard => dashboard::render(frame, &self.store, &self.config, &mut self.dashboard_state),
            View::History => history::render(frame, &self.store, &self.config, self.scroll),
            View::Sessions => sessions::render(frame, &self.store, &self.config, &mut self.sessions_state),
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
}
