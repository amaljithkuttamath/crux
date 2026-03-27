# crux

Terminal dashboard for AI coding session analytics (Claude Code + Cursor).

**Read `docs/vision.md` first for project direction and next moves.**

## Build & Run

```bash
cargo build --release        # binary at target/release/crux
cargo run --release           # launch TUI
cargo run -- summary          # CLI mode
cargo run -- stats            # activity stats, heatmap, achievements
cargo run -- budget           # context budget scanner
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
  budget.rs            # Context budget scanner + duplication analysis (filesystem, no AI)

  parser/
    mod.rs             # JSONL line parser, extract_project_name
    conversation.rs    # SessionMeta (lightweight), ConversationMessage (lazy), types
    cursor.rs          # SQLite parser for Cursor's state.vscdb (composerData, bubbleId tokens)
    watcher.rs         # notify-based file watcher for live CC updates

  store/
    mod.rs             # Store struct, 40+ aggregation methods
    analysis.rs        # SessionAnalysis, SessionTimeline, TurnSnapshot, grade_letter()
    cursor.rs          # CursorModelStat, CursorOverviewStats, cursor-specific aggregation

  tui/
    mod.rs             # App struct, event loop, View enum (Browser + Stats), key handling
    widgets.rs         # Color palette, shared helpers (truncate, spark, smooth_bar, compact, etc.)
    browser.rs         # Browser: today cockpit header, three-panel explorer, session detail overlay
    stats.rs           # Stats: heatmap, key numbers, 30d trend, daily table, models, badges, budget
    dashboard.rs       # Session detail view (context growth chart, health panel). Utility module.
    help.rs            # Help overlay

  cli/
    mod.rs             # CLI output: summary, daily, project, session, health, stats, budget

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
- `SessionMeta` - per-session summary: first_message, tools_used, duration, source, cursor-specific fields (model_name, subtitle, lines, todos, context_usage)
- `SessionAnalysis` - computed metrics: context_growth, cache_hit_rate, grade_letter(), cost_breakdown
- `SessionTimeline` - per-turn snapshots for detail view context charts

## View Structure (2 views)

| View | Key | Focus |
|------|-----|-------|
| Browser | default | Today cockpit header, three-panel explorer (projects/sessions/sidebar), session detail overlay |
| Stats | `s` | Heatmap hero, key numbers, 30d trend, daily cost table, models, badges, context budget |

Navigation: `b` Browser (all), `d` Browser+CC filter, `c` Browser+Cursor filter, `f` cycle source filter, `s` Stats. Arrow keys navigate panels. `Enter` opens session detail (context growth chart). `Right` opens conversation. `Esc` goes back. From Stats: `Esc`/`b` returns to Browser.

## Store Methods (grouped)

**Time aggregation:** today(), this_week(), all_time(), by_day(n), daily_costs(n), today_by_hour(), today_by_source(), rolling_avg_daily_cost()
**Grouping:** by_model(), by_project(), by_project_cost(), by_source(), top_tools(n)
**Session queries:** sessions_by_time(), sessions_by_source(), active_sessions(), search_sessions(), search_full_text(), session_cost(), session_tokens(), session_model(), session_meta(), session_timeline(), analyze_session()
**Subagent:** subagents_for(), session_model_mix()
**Stats:** streak_days(), longest_streak(), active_days(), total_tokens(), longest_session(), most_active_day(), favorite_model(), peak_hour(), hourly_distribution(), weekday_distribution(), night_owl_ratio(), avg_session_duration(), output_per_dollar()
**Efficiency:** grade_distribution(), avg_cache_hit_rate(), total_context_premium(), total_compactions(), session_duration_buckets()
**Comparison:** week_comparison(), month_comparison(), month_projection()
**Budget:** budget::scan() (filesystem scanner, separate module)

## Browser Sidebar Data

The sidebar renders different content based on source:

**Claude Code sessions:** grade, cost, duration, model, messages, context bar + growth + cache, token bars (in/out), cost breakdown, context sparkline, AGENTS section (tree of subagents with type/model/cost), MODEL MIX (proportional bars when multi-model), TOOLS
**Cursor sessions:** actual model name (from modelConfig), duration, messages, mode/status/agentic badge, subtitle (files edited), token bars (from bubble aggregation), context bar (from Cursor's own reporting), CHANGES (files edited/added/removed, lines added/removed), TODOS

## Cursor Data Sources

Cursor data comes from `state.vscdb` SQLite:
- `composerData:*` - session metadata: name, status, mode, modelConfig, timestamps, todos, lines, files, subtitle
- `bubbleId:*` - per-message data: tokenCount (inputTokens/outputTokens), text, type, isAgentic, supportedTools
- `aiCodeTracking.dailyStats.*` - daily: tabSuggestedLines, tabAcceptedLines, composerSuggestedLines, composerAcceptedLines
- `agentKv:*` - agent state (unexplored)
- `checkpointId:*` - file snapshots at checkpoints (unexplored)

## What's Done (2026-03-27 sessions)

**Session 1: Stats, Browser, Budget, Cursor enrichment**
- [x] Stats view: heatmap, streaks, badges, charts, efficiency, budget, records
- [x] Browser view: three-panel explorer, arrow-key navigation, live sidebar
- [x] Budget scanner: filesystem scan, token estimation, stale detection
- [x] Source filter (`f`), project filter (`p`), UTF-8 safe truncation
- [x] Store: 25+ new aggregation methods

**Session 2: Product consolidation (6 views to 2)**
- [x] Consolidated 6 views down to 2: Browser (default) + Stats (retrospective)
- [x] Removed: Overview, Claude Code, Cursor, History as standalone views
- [x] Deleted: sessions.rs, cursor_view.rs, history.rs (1,542 lines removed)
- [x] Browser: always-visible today cockpit header (cost/avg/burn rate/spark/streak/budget/source split)
- [x] Browser: `d`/`c` keys set source filter instead of switching views
- [x] Browser: Enter on session opens detail overlay (context growth chart from old Overview)
- [x] Stats: redesigned as clean vertical flow (heatmap, key numbers, trend, daily table, footer)
- [x] Stats: absorbed History content (cumulative trend, source split, daily cost table, model breakdown)
- [x] Stats: cut low-value charts (hourly, weekday, grade, tools, top projects)
- [x] Budget: duplication analysis (line-level comparison across always-loaded files, wasted token estimation)
- [x] CTX bar: compaction-aware coloring (yellow for compacted sessions, red for fresh near-limit)
- [x] Help bars: verbose descriptions since only 2 views now
- [x] Nav header: simplified to [B]rowser [S]tats

## What's Next

**Features:**
- [ ] Deep parse: tool arguments, file paths read/written per session (highest value unexplored data)
- [ ] Cursor daily AI code tracking: tab vs composer suggested/accepted lines
- [ ] Year-in-review / "Wrapped" screen
- [ ] Cache budget scan results (filesystem doesn't change mid-session)

**Architecture notes:**
- Full-text search (`search_full_text()`) reads JSONL lazily. Fine for small datasets, may need indexing for 500+ sessions.
- MCP server (5 tools) is self-contained, kept but not actively used. `should_restart` could become a hook.

## Data Inventory: Parsed vs Unexplored

### Claude Code JSONL - what we parse

| Field | Parsed? | Used in |
|-------|---------|---------|
| `type` (user/assistant/progress) | Yes | message counting, conversation parsing |
| `timestamp` | Yes | timelines, aggregation, sorting |
| `sessionId` | Yes | session grouping |
| `message.role` | Yes | user/assistant distinction |
| `message.content[].type` (text/tool_use) | Yes | tool counting, conversation preview |
| `message.content[].name` (tool name) | Yes | tool_counts in SessionMeta |
| `message.content[].text` | Yes | first_message, conversation preview |
| `message.model` | Yes | model breakdown, pricing |
| `message.usage.input_tokens` | Yes | UsageRecord, cost estimation |
| `message.usage.output_tokens` | Yes | UsageRecord, cost estimation |
| `message.usage.cache_creation_input_tokens` | Yes | UsageRecord, cache analysis |
| `message.usage.cache_read_input_tokens` | Yes | UsageRecord, cache analysis |
| `message.stop_reason` | No | Available: end_turn, tool_use, stop_sequence |
| `message.usage.speed` | No | Available: "standard" (could detect fast mode) |
| `message.usage.inference_geo` | No | Available: inference region |
| `message.usage.server_tool_use` | No | Available: server-side tool use flag |
| `message.usage.iterations` | No | Available: iteration count |
| `message.content[].input` (tool args) | **No** | **HIGH VALUE**: file paths read/written, bash commands, grep patterns, agent prompts |
| `message.content[] tool_result` | **No** | **HIGH VALUE**: result sizes, which tools returned large content |
| `entrypoint` (cli/sdk-cli) | No | Could distinguish interactive vs headless sessions |
| `gitBranch` | No | Could group sessions by branch |
| `cwd` | No | Working directory per message (could track cd patterns) |
| `version` | No | Claude Code version tracking |
| `isSidechain` | No | Sidechain conversation tracking |
| `parentUuid` / `uuid` | No | Message threading |
| Subagent `.meta.json` `agentType` | Yes | agent_type in SessionMeta |

### Cursor SQLite - what we parse

| Table/Key | Parsed? | Used in |
|-----------|---------|---------|
| `composerData:*` name | Yes | first_message |
| `composerData:*` status | Yes | cursor_status |
| `composerData:*` unifiedMode | Yes | cursor_mode |
| `composerData:*` createdAt/lastUpdatedAt | Yes | start/end time |
| `composerData:*` totalLinesAdded/Removed | Yes | lines_added/removed |
| `composerData:*` filesChangedCount | Yes | files_changed |
| `composerData:*` modelConfig.modelName | Yes | cursor_model_name, UsageRecord.model |
| `composerData:*` isAgentic | Yes | is_agentic |
| `composerData:*` todos | Yes | cursor_todos |
| `composerData:*` subtitle | Yes | cursor_subtitle |
| `composerData:*` addedFiles/removedFiles | Yes | added_files/removed_files |
| `composerData:*` fullConversationHeadersOnly | Yes | user/assistant count |
| `bubbleId:*` tokenCount.inputTokens | Yes | UsageRecord aggregation |
| `bubbleId:*` tokenCount.outputTokens | Yes | UsageRecord aggregation |
| `composerData:*` forceMode (chat/edit/plan) | No | Different from unifiedMode, shows user's requested mode |
| `composerData:*` usageData | No | Empty in current data, may populate in future |
| `composerData:*` capabilities | No | Feature flags per session |
| `composerData:*` context (composers, commits, PRs, images, folders) | No | What context was attached |
| `composerData:*` subComposerIds | No | Cursor's own subagent tracking |
| `composerData:*` isSpec/isProject | No | Spec mode / project mode flags |
| `composerData:*` checkpointId references | No | Link to file snapshots |
| `bubbleId:*` text | No | Full message text (conversation content) |
| `bubbleId:*` supportedTools (28 tools) | No | Which tools Cursor had available |
| `bubbleId:*` toolResults | No | Tool execution results |
| `bubbleId:*` docsReferences | No | Documentation context used |
| `bubbleId:*` webReferences / aiWebSearchResults | No | Web search context |
| `bubbleId:*` cursorRules | No | Which cursor rules were active |
| `bubbleId:*` isAgentic (per message) | No | Agentic state per turn (vs per session) |
| `bubbleId:*` fileDiffTrajectories | No | File change history per message |
| `bubbleId:*` checkpointId | No | Checkpoint per message |
| `bubbleId:*` allThinkingBlocks | No | Thinking/reasoning content |
| `agentKv:*` (7905 entries) | **No** | **UNEXPLORED**: agent state, possibly tool execution logs |
| `checkpointId:*` (2572 entries) | **No** | **UNEXPLORED**: file snapshots at checkpoints |
| `aiCodeTracking.dailyStats.*` | **No** | **HIGH VALUE**: tabSuggestedLines, tabAcceptedLines, composerSuggestedLines, composerAcceptedLines per day |
| `aiCodeTrackingLines` | **No** | Per-line tracking with file names and composer IDs |
| `aiCodeTrackingScoredCommits` | **No** | Commit scoring data |
| `codeBlockDiff:*` (1745 entries) | **No** | Code diffs generated by Cursor |
| `messageRequestContext:*` (626 entries) | **No** | Context sent with each request |

### Highest value unexplored data

1. **CC tool arguments** (`message.content[].input`): file paths, bash commands, grep patterns. Enables "files touched" sidebar, redundancy detection, file hotspot tracking.
2. **Cursor aiCodeTracking**: daily suggested vs accepted lines. Direct productivity metric.
3. **Cursor agentKv**: 7905 entries of agent state. Unknown structure, needs exploration.
4. **Cursor bubbleId text**: full conversation content for Cursor sessions. Would enable conversation preview in browser.
5. **CC entrypoint/gitBranch**: session grouping by interactive vs headless, and by git branch.

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
- All string truncation must use `.chars().count()` / `.chars().take(n)`, never byte slicing.
- No em dashes anywhere. Use commas, periods, or restructure.
