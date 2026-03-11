pub mod dashboard;
pub mod daily;
pub mod trends;
pub mod models;
pub mod insights;
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
    Daily,
    Trends,
    Models,
    Insights,
}

pub struct App {
    pub store: Store,
    pub config: Config,
    pub view: View,
    pub should_quit: bool,
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
                        match key.code {
                            KeyCode::Char('q') => self.should_quit = true,
                            KeyCode::Char('d') => self.view = View::Daily,
                            KeyCode::Char('t') => self.view = View::Trends,
                            KeyCode::Char('m') => self.view = View::Models,
                            KeyCode::Char('i') => self.view = View::Insights,
                            KeyCode::Esc => self.view = View::Dashboard,
                            _ => {}
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn draw(&self, frame: &mut ratatui::Frame) {
        match self.view {
            View::Dashboard => dashboard::render(frame, &self.store, &self.config),
            View::Daily => daily::render(frame, &self.store, &self.config),
            View::Trends => trends::render(frame, &self.store, &self.config),
            View::Models => models::render(frame, &self.store, &self.config),
            View::Insights => insights::render(frame, &self.store, &self.config),
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
