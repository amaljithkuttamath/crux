# crux

A terminal dashboard for understanding your AI coding sessions. Built for Claude Code and Cursor, designed to answer: **where do my tokens go?**

![Rust](https://img.shields.io/badge/rust-2021-orange) ![License](https://img.shields.io/badge/license-MIT-blue) [![crates.io](https://img.shields.io/crates/v/crux-tokens.svg)](https://crates.io/crates/crux-tokens)

## What it does

crux reads session data from Claude Code (JSONL logs) and Cursor IDE (SQLite database) and gives you a live, interactive terminal dashboard with:

- **Active sessions** with real-time context window fill, cache hit rate, efficiency grades (A-F), and compaction detection
- **Session timeline** drill-down showing context growth, cost spikes, and notable events
- **Daily/weekly trends** with token volume bar charts and model breakdowns
- **Session browser** with full conversation replay
- **MCP server** exposing 5 analysis tools for session health, cost breakdown, and restart recommendations

## Install

### Homebrew (macOS and Linux)

```bash
brew install amaljithkuttamath/tap/crux
```

### Cargo

```bash
cargo install crux-tokens
```

### From release binaries

Download the latest binary for your platform from [Releases](https://github.com/amaljithkuttamath/crux/releases).

### From source

```bash
git clone https://github.com/amaljithkuttamath/crux.git
cd crux
cargo build --release
# binary at target/release/crux
```

## Usage

### Interactive dashboard

```bash
crux
```

The dashboard has three views:

| View | Key | What it shows |
|------|-----|---------------|
| **Dashboard** | (default) | Active sessions, project breakdown, efficiency grades |
| **History** | `h` | Daily/weekly token trends, model usage over time |
| **Sessions** | `s` | Browse all sessions, drill into conversation replay |

### Navigation

| Key | Action |
|-----|--------|
| `h` | Switch to History view |
| `s` | Switch to Sessions view |
| `Tab` | Toggle focus between active sessions and projects (Dashboard) |
| `Enter` | Drill into session detail / context timeline |
| `Esc` | Back to previous view |
| `j` / `k` | Scroll down / up |
| `q` | Quit |

### CLI commands

For quick lookups without the TUI:

```bash
crux summary     # today's totals, one line
crux daily       # last 7 days table
crux project     # per-project breakdown
crux session     # list sessions with token counts
```

### MCP server

Run crux as an MCP server for Claude Code to query your session analytics mid-conversation:

```bash
crux serve
```

This exposes 5 tools over stdio:

- `session_health` - real-time session metrics and efficiency grade
- `session_cost` - detailed cost breakdown with context growth premium
- `should_restart` - recommendation engine for when to start fresh
- `list_sessions` - browse recent sessions with filters
- `search_sessions` - keyword search across session topics

Add to your Claude Code config (`~/.claude.json`):

```json
{
  "mcpServers": {
    "crux": {
      "command": "crux",
      "args": ["serve"]
    }
  }
}
```

## Configuration

Config lives at `~/.config/crux/config.toml`:

```toml
# Budget tracking (optional)
budget_daily = 5.00
budget_weekly = 25.00

# Exclude projects from tracking
exclude_projects = ["test-project"]

# Insights sparkline range
insights_sparkline_days = 14

# Cursor IDE (auto-detected, enabled by default)
enable_cursor = true
# cursor_data_path = "~/Library/Application Support/Cursor/User/globalStorage/state.vscdb"
```

## How it works

**Claude Code:** crux parses the JSONL session logs that Claude Code writes to `~/.claude/projects/`. Each API call includes token counts (input, output, cache read, cache write) and model info.

**Cursor:** crux reads Cursor's SQLite database at `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`, extracting session metadata and per-message token counts across all models (Claude, GPT, Grok, Gemini, etc.).

Both sources feed into the same analytics pipeline. crux aggregates into session-level analytics, detects context window compactions, calculates efficiency metrics, and renders everything in a ratatui-powered TUI.

Key metrics:
- **Context growth factor** - how much your context window expanded from start to current
- **Cache hit rate** - what percentage of input comes from cache (higher = cheaper)
- **Output efficiency** - ratio of useful output to total context processed
- **Context growth premium** - extra cost from expanding context vs. starting fresh
- **Efficiency grade** - A-F composite score based on growth, efficiency, and cost

## License

MIT
