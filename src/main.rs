use clap::{Parser, Subcommand};

mod cli;
mod config;
mod parser;
mod pricing;
mod store;
mod tui;

use config::Config;
use store::Store;

#[derive(Parser)]
#[command(
    name = "usagetracker",
    about = "Terminal dashboard for AI coding tool token usage"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Today's totals, one line
    Summary,
    /// Last 7 days table
    Daily,
    /// Per-project breakdown
    Project,
    /// List sessions with token counts
    Session,
}

fn main() -> anyhow::Result<()> {
    let cli_args = Cli::parse();
    let config = Config::load();
    let store = load_store(&config)?;

    match cli_args.command {
        Some(Commands::Summary) => print!("{}", cli::format_summary(&store)),
        Some(Commands::Daily) => print!("{}", cli::format_daily(&store, 7)),
        Some(Commands::Project) => print!("{}", cli::format_projects(&store)),
        Some(Commands::Session) => print!("{}", cli::format_sessions(&store)),
        None => {
            let terminal = ratatui::init();
            let result = tui::App::new(store, config).run(terminal);
            ratatui::restore();
            result?;
        }
    }
    Ok(())
}

fn load_store(config: &Config) -> anyhow::Result<Store> {
    let mut store = Store::new();
    for data_dir in config.all_data_dirs() {
        if !data_dir.exists() {
            continue;
        }
        for project_dir in std::fs::read_dir(&data_dir)? {
            let project_dir = project_dir?.path();
            if !project_dir.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(&project_dir)? {
                let path = entry?.path();
                if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    if let Ok(records) = parser::parse_file(path.to_str().unwrap_or_default()) {
                        for r in records {
                            if !config.is_excluded(&r.project) {
                                store.add(r);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(store)
}
