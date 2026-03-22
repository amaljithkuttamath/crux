use clap::{Parser, Subcommand};
use rmcp::ServiceExt;

mod cli;
mod config;
mod mcp;
mod parser;
mod pricing;
mod store;
mod tui;

use config::Config;
use store::Store;

#[derive(Parser)]
#[command(
    name = "crux",
    about = "Terminal dashboard and MCP session analyst for AI coding tool token usage"
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
    /// Active session health for scripting
    Health,
    /// Run as MCP server over stdio
    Serve,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli_args = Cli::parse();
    let config = Config::load();
    let store = load_store(&config)?;

    match cli_args.command {
        Some(Commands::Summary) => print!("{}", cli::format_summary(&store)),
        Some(Commands::Daily) => print!("{}", cli::format_daily(&store, 7)),
        Some(Commands::Project) => print!("{}", cli::format_projects(&store)),
        Some(Commands::Session) => print!("{}", cli::format_sessions(&store)),
        Some(Commands::Health) => print!("{}", cli::format_health(&store)),
        Some(Commands::Serve) => {
            let server = mcp::UsageServer::new(store, config);
            let service = server
                .serve(rmcp::transport::stdio())
                .await?;
            service.waiting().await?;
        }
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
    let mut store = Store::default();

    // Load Claude Code JSONL files
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
                    let path_str = path.to_str().unwrap_or_default();
                    if let Ok(records) = parser::parse_file(path_str) {
                        for r in records {
                            if !config.is_excluded(&r.project) {
                                store.add(r);
                            }
                        }
                    }
                    // Load lightweight session metadata
                    if let Ok(meta) = parser::conversation::parse_session_meta(path_str) {
                        if meta.user_count > 0 && !config.is_excluded(&meta.project) {
                            store.add_session_meta(meta);
                        }
                    }
                }
            }

            // Parse subagent directories
            for entry in std::fs::read_dir(&project_dir)? {
                let session_dir = entry?.path();
                if !session_dir.is_dir() { continue; }
                let subagents_dir = session_dir.join("subagents");
                if !subagents_dir.is_dir() { continue; }

                let parent_session_id = session_dir.file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from);

                for sub_entry in std::fs::read_dir(&subagents_dir)? {
                    let sub_path = sub_entry?.path();
                    if sub_path.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }
                    let sub_path_str = sub_path.to_str().unwrap_or_default();

                    if let Ok(records) = parser::parse_file(sub_path_str) {
                        for r in records {
                            if !config.is_excluded(&r.project) {
                                store.add(r);
                            }
                        }
                    }

                    if let Ok(mut meta) = parser::conversation::parse_session_meta(sub_path_str) {
                        meta.parent_session_id = parent_session_id.clone();
                        meta.is_subagent = true;

                        // Read agent type from .meta.json (e.g. agent-abc123.meta.json)
                        let stem = sub_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let meta_json = subagents_dir.join(format!("{}.meta.json", stem));
                        if meta_json.exists() {
                            if let Ok(content) = std::fs::read_to_string(&meta_json) {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                    meta.agent_type = json.get("agentType")
                                        .and_then(|v| v.as_str())
                                        .map(String::from);
                                }
                            }
                        }

                        if meta.user_count > 0 && !config.is_excluded(&meta.project) {
                            store.add_session_meta(meta);
                        }
                    }
                }
            }
        }
    }

    // Load Cursor sessions
    if let Some(cursor_path) = config.cursor_db_path() {
        if let Some(path_str) = cursor_path.to_str() {
            match parser::cursor::parse_cursor_db(path_str) {
                Ok((records, metas)) => {
                    for r in records {
                        if !config.is_excluded(&r.project) {
                            store.add(r);
                        }
                    }
                    for m in metas {
                        if m.user_count > 0 && !config.is_excluded(&m.project) {
                            store.add_session_meta(m);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: failed to load Cursor data: {}", e);
                }
            }
        }
    }

    Ok(store)
}
