use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub rolling_window: String,
    pub refresh_interval: String,
    pub data_path: String,
    pub accent_color: String,
    pub compact_numbers: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rolling_window: "5h".to_string(),
            refresh_interval: "2s".to_string(),
            data_path: "~/.claude/projects".to_string(),
            accent_color: "copper".to_string(),
            compact_numbers: true,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn data_dir(&self) -> PathBuf {
        let expanded = shellexpand(&self.data_path);
        PathBuf::from(expanded)
    }

    pub fn rolling_window_duration(&self) -> chrono::Duration {
        parse_duration(&self.rolling_window).unwrap_or(chrono::Duration::hours(5))
    }

    pub fn refresh_interval_duration(&self) -> std::time::Duration {
        parse_std_duration(&self.refresh_interval).unwrap_or(std::time::Duration::from_secs(2))
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("usagetracker")
        .join("config.toml")
}

fn shellexpand(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &path[1..]);
        }
    }
    path.to_string()
}

fn parse_duration(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    if let Some(h) = s.strip_suffix('h') {
        h.parse::<i64>().ok().map(chrono::Duration::hours)
    } else if let Some(m) = s.strip_suffix('m') {
        m.parse::<i64>().ok().map(chrono::Duration::minutes)
    } else {
        None
    }
}

fn parse_std_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if let Some(ms) = s.strip_suffix("ms") {
        ms.parse::<u64>().ok().map(std::time::Duration::from_millis)
    } else if let Some(secs) = s.strip_suffix('s') {
        secs.parse::<u64>().ok().map(std::time::Duration::from_secs)
    } else {
        None
    }
}
