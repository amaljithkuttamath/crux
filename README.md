# crux

A terminal dashboard for understanding your AI coding sessions. Built for Claude Code and Cursor, designed to answer: **where do my tokens go?**

![Rust](https://img.shields.io/badge/rust-2021-orange) ![License](https://img.shields.io/badge/license-MIT-blue) [![crates.io](https://img.shields.io/crates/v/crux-tokens.svg)](https://crates.io/crates/crux-tokens)

## What it does

crux reads session data from Claude Code (JSONL logs) and Cursor IDE (SQLite database) and gives you:

**Browser** (default view) - interactive session explorer:

- Today cockpit: cost, burn rate ($/hr), vs 7d avg, streak, budget fill, CC/Cursor source split
- Three-panel explorer: projects, sessions, live stats sidebar
- Source filtering: `d` for Claude Code only, `c` for Cursor only, `f` to cycle
- Session detail overlay: context growth chart, health panel, cost breakdown, activity pattern
- Conversation replay with tool use timeline

**Stats** - retrospective analytics:

- Contribution heatmap (GitHub-style, with cost sparkline)
- Key numbers: sessions, streak, tokens, cost, cache rate, compactions, efficiency
- 30-day cumulative trend with CC/Cursor source split
- Scrollable daily cost table
- Model breakdown, badges, context budget with duplication analysis

**Menu bar monitor (macOS)** - at-a-glance session health without leaving your editor

**MCP server** - 5 analysis tools for session health, cost breakdown, and restart recommendations

**CLI** - scriptable session analytics for automation and piping

## Install

### Homebrew (macOS and Linux)

```bash
brew install amaljithkuttamath/tap/crux
```

### Cargo

```bash
cargo install crux-tokens
```

### From source

```bash
git clone https://github.com/amaljithkuttamath/crux.git
cd crux
cargo build --release
# binary at target/release/crux

# Menu bar app (macOS only, requires Swift 5.9+)
cd crux-bar
swift build -c release
# binary at .build/release/CruxBar
```

## Usage

### Interactive dashboard

```bash
crux
```

Two views, keyboard-driven:

| View | Keys | What it shows |
|------|------|---------------|
| **Browser** | default, `b` | Today cockpit, three-panel session explorer, detail overlay |
| **Stats** | `s` | Heatmap, key numbers, trends, daily table, models, badges |

### Navigation

| Key | Action |
|-----|--------|
| `b` | Browser (all sources) |
| `d` | Browser, Claude Code only |
| `c` | Browser, Cursor only |
| `f` | Cycle source filter (All/CC/Cursor) |
| `s` | Stats view |
| `Enter` | Session detail (context growth chart) |
| `Right` | Drill into conversation |
| `Left` / `Esc` | Back |
| `j` / `k` | Navigate / scroll |
| `/` | Search sessions |
| `?` | Help |
| `q` | Quit |

### Menu bar monitor (macOS)

After installing via Homebrew:

```bash
open $(brew --prefix)/CruxBar.app
```

CruxBar runs in your menu bar showing today's cost and active session count. Click for the full popover with health grades and context fill.

To start at login: System Settings > General > Login Items > add CruxBar.

### CLI commands

```bash
crux summary          # today's cost with breakdown
crux daily            # last 7 days with cost, tokens, sessions
crux project          # per-project breakdown
crux session          # recent sessions with grade and source
crux health           # active session health (FRESH/OK/AGING/CRITICAL)
crux stats            # activity stats, heatmap, achievements
crux budget           # context budget scan with duplication analysis
crux serve            # MCP server (5 tools over stdio)
```

### MCP server

Run crux as an MCP server for Claude Code to query your session analytics mid-conversation:

```bash
crux serve
```

Tools: `session_health`, `session_cost`, `should_restart`, `list_sessions`, `search_sessions`

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

# Cursor IDE (auto-detected, enabled by default)
enable_cursor = true
# cursor_data_path = "~/Library/Application Support/Cursor/User/globalStorage/state.vscdb"
```

## How it works

**Claude Code:** crux parses the JSONL session logs at `~/.claude/projects/`. Each API call includes token counts (input, output, cache read, cache write) and model info. A file watcher detects new records in real time.

**Cursor:** crux reads Cursor's SQLite database at `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`, extracting session metadata, per-message token counts, completion status, lines shipped, and context fill. Refreshes every 30 seconds.

Both sources feed into the same analytics pipeline: session-level aggregation, compaction detection, efficiency metrics, and a ratatui-powered TUI.

Key metrics:
- **Context growth factor** - how much your context window expanded from start
- **Cache hit rate** - percentage of input from cache (higher = cheaper)
- **Output efficiency** - ratio of useful output to total context processed
- **Context growth premium** - extra cost from growing context vs. starting fresh
- **Efficiency grade** - A-F composite score
- **Session health** - fresh/healthy/aging/ctx rot based on context fill and growth
- **Duplication analysis** - percentage of always-loaded context that's redundant across files

## License

MIT
