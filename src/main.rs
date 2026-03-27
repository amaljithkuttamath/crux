use clap::{Parser, Subcommand};
use rmcp::ServiceExt;

use crux::{cli, config, mcp, tui};
use config::Config;

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
    /// Export widget JSON for menu bar app
    ExportWidget {
        /// Re-export every 60s instead of one-shot
        #[arg(long)]
        watch: bool,
    },
    /// Output ANSI-colored status line (reads Claude Code session JSON from stdin)
    Statusline,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli_args = Cli::parse();
    let config = Config::load();

    // Statusline reads widget.json first. Only loads store as last-resort fallback.
    if let Some(Commands::Statusline) = &cli_args.command {
        return cli::statusline::run_statusline(&config);
    }

    let store = crux::load_store(&config)?;

    match cli_args.command {
        Some(Commands::Summary) => print!("{}", cli::format_summary(&store)),
        Some(Commands::Daily) => print!("{}", cli::format_daily(&store, 7)),
        Some(Commands::Project) => print!("{}", cli::format_projects(&store)),
        Some(Commands::Session) => print!("{}", cli::format_sessions(&store)),
        Some(Commands::Health) => print!("{}", cli::format_health(&store)),
        Some(Commands::ExportWidget { watch }) => {
            if watch {
                cli::widget::export_watch(&config)?;
            } else {
                cli::widget::export_once(&store, &config)?;
            }
        }
        Some(Commands::Serve) => {
            let server = mcp::UsageServer::new(store, config);
            let service = server
                .serve(rmcp::transport::stdio())
                .await?;
            service.waiting().await?;
        }
        Some(Commands::Statusline) => unreachable!(),
        None => {
            let terminal = ratatui::init();
            let result = tui::App::new(store, config).run(terminal);
            ratatui::restore();
            result?;
        }
    }
    Ok(())
}


