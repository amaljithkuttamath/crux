# crux

A terminal dashboard for understanding your AI coding sessions. Built for Claude Code, designed to answer: **where do my tokens go?**

![Rust](https://img.shields.io/badge/rust-2021-orange) ![License](https://img.shields.io/badge/license-MIT-blue)

## What it does

crux reads Claude Code's session logs and gives you a live, interactive terminal dashboard with:

- **Active sessions** with real-time context window fill, cache hit rate, efficiency grades (A-F), and compaction detection
- **Session timeline** drill-down showing context growth, cost spikes, and notable events
- **Daily/weekly trends** with token volume bar charts and model breakdowns
- **Insights view** with cache efficiency, output ratios, 24h activity heatmaps, and heaviest sessions
- **Session browser** with full conversation replay
- **MCP server** exposing 5 analysis tools for session health, cost breakdown, and restart recommendations

## Install

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/amaljithkuttamath/crux.git
cd crux
cargo build --release
# binary at target/release/crux
```

## Usage

```bash
# Launch the interactive dashboard
crux

# Quick summaries
crux summary     # today's totals, one line
crux daily       # last 7 days table
crux project     # per-project breakdown
crux session     # list sessions with token counts

# Run as MCP server (for integration with Claude Code)
crux serve
```

### Dashboard navigation

| Key | Action |
|-----|--------|
| `Tab` | Switch between active sessions and projects |
| `Enter` | Drill into session detail / context timeline |
| `Esc` | Back |
| `d` | Daily view |
| `t` | Trends view |
| `m` | Models view |
| `i` | Insights view |
| `s` | Sessions browser |
| `q` | Quit |

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
```

## MCP server

crux exposes 5 tools over MCP for querying session analytics programmatically:

- `session_health` - real-time session metrics and efficiency grade
- `session_cost` - detailed cost breakdown with context growth premium
- `should_restart` - recommendation engine for when to start fresh
- `list_sessions` - browse recent sessions with filters
- `search_sessions` - keyword search across session topics

Add to your Claude Code MCP config:

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

## How it works

crux parses the JSONL session logs that Claude Code writes to `~/.claude/projects/`. Each API call includes token counts (input, output, cache read, cache write) and model info. crux aggregates these into session-level analytics, detects context window compactions, calculates efficiency metrics, and renders everything in a ratatui-powered TUI.

Key metrics:
- **Context growth factor** - how much your context window expanded from start to current
- **Cache hit rate** - what percentage of input comes from cache (higher = cheaper)
- **Output efficiency** - ratio of useful output to total context processed
- **Context growth premium** - extra cost from expanding context vs. starting fresh
- **Efficiency grade** - A-F composite score based on growth, efficiency, and cost

## License

MIT
