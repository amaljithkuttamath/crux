# crux

A terminal dashboard for understanding your AI coding sessions. Built for Claude Code and Cursor, designed to answer: **where do my tokens go?**

![Rust](https://img.shields.io/badge/rust-2021-orange) ![License](https://img.shields.io/badge/license-MIT-blue) [![crates.io](https://img.shields.io/crates/v/crux-tokens.svg)](https://crates.io/crates/crux-tokens)

## What it does

crux reads session data from Claude Code (JSONL logs) and Cursor IDE (SQLite database) and gives you:

**Menu bar monitor (macOS)** - at-a-glance session health without leaving your editor:

- Today's cost and burn rate ($/hr)
- Active sessions with health grades (A-F) and context fill
- Claude Code + Cursor cost split

**Terminal dashboard** - full interactive TUI:

- **Overview** with ticker bar (cost, burn rate, 7d sparkline, streak), active session health with context trajectory sparklines, and split Claude Code / Cursor panes
- **Claude Code view** with daily cost bars, model breakdown (opus/sonnet/haiku), date-grouped session list with context sparklines and efficiency grades
- **Cursor view** with model comparison bars, session list with mode badges and line counts, session detail with todo display
- **History view** with 30-day cumulative trend, CC vs Cursor source split, daily cost table, and model breakdown
- **Session drill-down** with context timeline, cost sparkline, conversation replay, and cost breakdown

**MCP server** - 5 analysis tools for session health, cost breakdown, and restart recommendations

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

### Menu bar monitor (macOS)

After installing via Homebrew:

```bash
open $(brew --prefix)/CruxBar.app
```

Or from source:

```bash
./crux-bar/.build/release/CruxBar
```

CruxBar runs in your menu bar and shows today's cost, active session count, and a green pulse dot. Click to see the full popover with session health grades, burn rate, and context fill per session.

CruxBar automatically spawns `crux export-widget --watch` in the background to keep data fresh. No manual setup needed.

To start at login: System Settings > General > Login Items > add CruxBar.

### Interactive dashboard

```bash
crux
```

The dashboard has four views:

| View | Key | What it shows |
|------|-----|---------------|
| **Overview** | (default) | Ticker bar, active sessions with health, split CC/Cursor panes |
| **Claude Code** | `d` | Daily cost bars, model breakdown, all CC sessions with grades |
| **Cursor** | `c` | Model comparison, unified session list with mode/status/lines |
| **History** | `h` | Cumulative trend, source split, daily costs, model breakdown |

### Navigation

| Key | Action |
|-----|--------|
| `d` | Claude Code view |
| `c` | Cursor view |
| `h` | History view |
| `Tab` | Switch pane focus (Overview) |
| `Enter` | Drill into session detail |
| `Esc` | Back to previous view |
| `j` / `k` | Scroll down / up |
| `q` | Quit |

### CLI commands

```bash
crux summary          # today's cost across both tools with breakdown
crux daily            # last 7 days with cost, tokens, sessions
crux project          # per-project breakdown with clean names
crux session          # list 30 most recent sessions with grade and source
crux health           # active session health for scripting (FRESH/OK/AGING/CRITICAL)
crux export-widget    # write ~/.cache/crux/widget.json (one-shot)
crux export-widget --watch  # re-export every 60s (used by CruxBar)
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

# Cursor IDE (auto-detected, enabled by default)
enable_cursor = true
# cursor_data_path = "~/Library/Application Support/Cursor/User/globalStorage/state.vscdb"
```

## How it works

**Claude Code:** crux parses the JSONL session logs that Claude Code writes to `~/.claude/projects/`. Each API call includes token counts (input, output, cache read, cache write) and model info. A file watcher detects new records in real time.

**Cursor:** crux reads Cursor's SQLite database at `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`, extracting session metadata, per-message token counts, completion status, lines shipped, and context fill across all models. Refreshes every 30 seconds.

Both sources feed into the same analytics pipeline. crux aggregates into session-level analytics, detects context window compactions, calculates efficiency metrics, and renders everything in a ratatui-powered TUI.

Key metrics:
- **Context growth factor** - how much your context window expanded from start to current
- **Cache hit rate** - what percentage of input comes from cache (higher = cheaper)
- **Output efficiency** - ratio of useful output to total context processed
- **Context growth premium** - extra cost from expanding context vs. starting fresh
- **Efficiency grade** - A-F composite score based on growth, efficiency, and cost
- **Session health** - FRESH/OK/AGING/CRITICAL based on context fill and growth trajectory

## License

MIT
