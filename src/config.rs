use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub rolling_window: String,
    pub refresh_interval: String,
    pub data_path: String,
    pub accent_color: String,
    pub compact_numbers: bool,

    // Budgets
    pub budget_daily: Option<f64>,
    pub budget_weekly: Option<f64>,

    // Thresholds
    pub rot_threshold: f64,
    pub cache_alert_ratio: f64,

    // Filters
    pub exclude_projects: Vec<String>,
    pub sort_projects_by: String,

    // Display
    pub sparkline_days: usize,
    pub peak_hours: Option<String>,   // e.g. "09-17"

    // Custom pricing overrides (per 1M tokens)
    pub model_costs: HashMap<String, ModelCostOverride>,

    // Additional watch paths
    pub watch_paths: Vec<String>,

    // Cursor IDE integration
    pub enable_cursor: bool,
    pub cursor_data_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ModelCostOverride {
    pub input: f64,
    pub output: f64,
    #[serde(default)]
    pub cache_write: Option<f64>,
    #[serde(default)]
    pub cache_read: Option<f64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rolling_window: "5h".to_string(),
            refresh_interval: "2s".to_string(),
            data_path: "~/.claude/projects".to_string(),
            accent_color: "copper".to_string(),
            compact_numbers: true,
            budget_daily: None,
            budget_weekly: None,
            rot_threshold: 5.0,
            cache_alert_ratio: 0.3,
            exclude_projects: Vec::new(),
            sort_projects_by: "recent".to_string(),
            sparkline_days: 7,
            peak_hours: None,
            model_costs: HashMap::new(),
            watch_paths: Vec::new(),
            enable_cursor: true,
            cursor_data_path: None,
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

    pub fn all_data_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.data_dir()];
        for p in &self.watch_paths {
            dirs.push(PathBuf::from(shellexpand(p)));
        }
        dirs
    }

    #[allow(dead_code)]
    pub fn rolling_window_duration(&self) -> chrono::Duration {
        parse_duration(&self.rolling_window).unwrap_or(chrono::Duration::hours(5))
    }

    pub fn refresh_interval_duration(&self) -> std::time::Duration {
        parse_std_duration(&self.refresh_interval).unwrap_or(std::time::Duration::from_secs(2))
    }

    /// Returns the Cursor state.vscdb path if Cursor is enabled and the file exists.
    pub fn cursor_db_path(&self) -> Option<PathBuf> {
        if !self.enable_cursor { return None; }
        let path = match &self.cursor_data_path {
            Some(p) => PathBuf::from(shellexpand(p)),
            None => default_cursor_path(),
        };
        if path.exists() { Some(path) } else { None }
    }

    pub fn is_excluded(&self, project: &str) -> bool {
        self.exclude_projects.iter().any(|e| {
            project.eq_ignore_ascii_case(e) || project.contains(e.as_str())
        })
    }

}

fn default_cursor_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        dirs::home_dir()
            .unwrap_or_default()
            .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb")
    } else {
        // Linux
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("Cursor/User/globalStorage/state.vscdb")
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("crux")
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
    } else if let Some(d) = s.strip_suffix('d') {
        d.parse::<i64>().ok().map(chrono::Duration::days)
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
