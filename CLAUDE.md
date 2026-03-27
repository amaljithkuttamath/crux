# crux

Terminal dashboard for AI coding session analytics (Claude Code + Cursor).

**Read `docs/vision.md` first for project direction and next moves.**

## Build & Run

```bash
cargo build --release        # binary at target/release/crux
cargo run --release           # launch TUI
cargo run -- summary          # CLI mode
cargo run -- serve            # MCP server
```

No tests yet. Validate changes with `cargo build --release` (zero warnings required).

## Architecture

```
src/
  main.rs              # CLI args (clap), store loading, dispatch
  config.rs            # TOML config from ~/.config/crux/config.toml
  pricing.rs           # Per-model cost estimation (input/output/cache pricing)
  lib.rs               # Crate root (re-exports)

  parser/
    mod.rs             # JSONL line parser, extract_project_name
    conversation.rs    # SessionMeta (lightweight), ConversationMessage (lazy), types
    cursor.rs          # SQLite parser for Cursor's state.vscdb
    watcher.rs         # notify-based file watcher for live CC updates

  store/
    mod.rs             # Store struct, aggregation methods (today/week/by_day/by_project/by_model)
    analysis.rs        # SessionAnalysis, SessionTimeline, TurnSnapshot, grade_letter()
    cursor.rs          # CursorModelStat, CursorOverviewStats, cursor-specific aggregation

  tui/
    mod.rs             # App struct, event loop, View enum, key handling, cursor refresh timer
    widgets.rs         # Color palette, shared helpers (truncate, spark, smooth_bar, compact, etc.)
    dashboard.rs       # View 1: Overview (ticker, active sessions, split CC/Cursor panes)
    sessions.rs        # View 2: Claude Code full view (daily bars, model breakdown, session list)
    cursor_view.rs     # View 3: Cursor unified view (model comparison, session list, detail)
    history.rs         # View 4: History (cumulative trend, source split, daily table, models)

  cli/
    mod.rs             # CLI output: summary, daily, project, session, health

  mcp/
    mod.rs             # MCP server (rmcp) with 5 tools
    tools.rs           # Tool definitions
```

## Data Flow

1. `main.rs` loads all Claude Code JSONL files from `~/.claude/projects/` and Cursor SQLite
2. Each file produces `Vec<UsageRecord>` (token counts) and `SessionMeta` (lightweight metadata)
3. `Store` holds all records + metas in memory, provides aggregation methods
4. TUI renders from Store. File watcher adds new CC records live. 30s timer refreshes Cursor data.

## Key Types

- `UsageRecord` - single API call: timestamp, session_id, project, model, token counts, source
- `SessionMeta` - per-session summary: first_message, tools_used, duration, cursor-specific fields
- `SessionAnalysis` - computed metrics: context_growth, cache_hit_rate, grade_letter(), cost_breakdown
- `SessionTimeline` - per-turn snapshots for detail view context charts

## View Structure (4 views)

| View | Key | Focus |
|------|-----|-------|
| Overview | default | Ambient display: ticker, active sessions, split panes |
| Claude Code | `d` | Full CC detail: daily cost bars, model breakdown, all sessions |
| Cursor | `c` | Full Cursor: model comparison, unified session list |
| History | `h` | Long view: cumulative trend, source split, daily costs, models |

Navigation: `d`/`c`/`h` from any view. `Enter` drills into session detail. `Esc` goes back. `Tab` switches pane focus on Overview.

## Color Palette (warm mineral)

| Name | RGB | Use |
|------|-----|-----|
| ACCENT | 224,155,95 | Headers, highlights, keys (warm amber) |
| ACCENT2 | 140,180,160 | Claude Code badge, haiku model (sage green) |
| FG | 240,234,226 | Primary text |
| FG_MUTED | 175,168,158 | Secondary text |
| FG_FAINT | 105,100,92 | Borders, labels, inactive |
| GREEN | 130,195,130 | Healthy, completed, lines added |
| YELLOW | 235,195,85 | Warnings, compactions |
| RED | 225,95,85 | Critical, aborted, lines removed |
| BLUE | 120,160,210 | Cursor badge, Cursor charts |
| PURPLE | 170,140,200 | Agent mode, opus model |

## Conventions

- `widgets.rs` owns all shared display helpers. Don't duplicate truncate/spark/compact/etc.
- `grade_letter()` is a method on `SessionAnalysis`, not a free function.
- `display_project_name()` cleans raw directory slugs for display. Applied at render time, not storage.
- Store methods return owned/borrowed data. Views never mutate Store.
- CLI module imports display helpers from `tui::widgets`.
- No em dashes anywhere. Use commas, periods, or restructure.
