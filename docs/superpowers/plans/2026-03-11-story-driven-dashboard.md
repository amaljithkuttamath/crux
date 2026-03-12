# Story-Driven Dashboard Redesign

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the usagetracker TUI to tell a visual story with compact charts, subscription-framed metrics, and agent spawn tracking, without overwhelming the user.

**Architecture:** Replace cost-as-headline framing with sessions/streak/grades. Add 2-column layout to dashboard summary. Detect Agent tool calls in JSONL parser for subagent tracking. Use Unicode horizontal bars and sparklines (no ratatui BarChart/Canvas widgets). Keep the existing Paragraph+Span composition approach.

**Tech Stack:** Rust, ratatui 0.29 (Sparkline, Gauge widgets), chrono, serde_json

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/tui/widgets.rs` | Modify | Add `LayoutMode`, `smooth_bar()`, `spark()` (moved from insights), `help_bar()` |
| `src/tui/dashboard.rs` | Modify | 2-column summary, new headline (sessions/streak), agent count on active sessions, cost-per-turn sparkline in detail |
| `src/tui/insights.rs` | Modify | Efficiency bars, 24h activity chart, heaviest sessions (not costliest), honest framing |
| `src/store.rs` | Modify | Add `streak_days()`, `sessions_per_day()`, `by_hour_all()`, `weekly_value_score()` |
| `src/parser/conversation.rs` | Modify | Detect `Agent` tool_use calls, count subagent spawns per session |
| `src/parser/mod.rs` | No change | -- |
| `src/config.rs` | No change | -- |
| `src/pricing.rs` | No change | -- |
| `src/tui/mod.rs` | No change | -- |
| `src/tui/trends.rs` | No change (future phase) | -- |

---

## Chunk 1: Widget Infrastructure + Store Methods

### Task 1: Move `spark()` to widgets.rs and make it public

**Files:**
- Modify: `src/tui/widgets.rs`
- Modify: `src/tui/insights.rs`

- [ ] **Step 1: Add `spark()` to widgets.rs**

Copy the spark function from insights.rs to widgets.rs and make it pub:

```rust
// Add to src/tui/widgets.rs after the divider() function

/// Sparkline from float values using Unicode block characters
pub fn spark(values: &[f64]) -> String {
    let blocks = ['_', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];
    let max = values.iter().cloned().fold(0.0f64, f64::max);
    if max <= 0.0 {
        return "_".repeat(values.len());
    }
    values.iter().map(|v| {
        let idx = ((v / max) * 8.0).round() as usize;
        blocks[idx.min(8)]
    }).collect()
}
```

- [ ] **Step 2: Remove `spark()` from insights.rs, use widget import**

In `src/tui/insights.rs`, delete the `fn spark()` function (lines 210-220). The `use super::widgets::*` import already covers it.

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles with no errors. The `spark()` call in `insights.rs:29` now resolves to `widgets::spark`.

- [ ] **Step 4: Commit**

```bash
git add src/tui/widgets.rs src/tui/insights.rs
git commit -m "refactor: move spark() to widgets for reuse across views"
```

### Task 2: Add `smooth_bar()` helper to widgets.rs

**Files:**
- Modify: `src/tui/widgets.rs`

- [ ] **Step 1: Add the smooth_bar function**

```rust
// Add to src/tui/widgets.rs after spark()

/// Horizontal bar with sub-character precision using Unicode 8th blocks.
/// Returns (filled_string, empty_string) for styled rendering.
pub fn smooth_bar(value: f64, max: f64, width: usize) -> (String, String) {
    if max <= 0.0 || width == 0 {
        return (String::new(), "\u{2591}".repeat(width));
    }
    let ratio = (value / max).clamp(0.0, 1.0);
    let total_eighths = (ratio * width as f64 * 8.0).round() as usize;
    let full_blocks = total_eighths / 8;
    let remainder = total_eighths % 8;

    // ▏▎▍▌▋▊▉█  (U+258F down to U+2588)
    let partials = [' ', '\u{258F}', '\u{258E}', '\u{258D}', '\u{258C}', '\u{258B}', '\u{258A}', '\u{2589}'];

    let mut filled = "\u{2588}".repeat(full_blocks);
    let empty_start;
    if remainder > 0 && full_blocks < width {
        filled.push(partials[remainder]);
        empty_start = full_blocks + 1;
    } else {
        empty_start = full_blocks;
    }
    let empty = "\u{2591}".repeat(width.saturating_sub(empty_start));
    (filled, empty)
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets.rs
git commit -m "feat: add smooth_bar() with sub-character precision"
```

### Task 3: Add `help_bar()` to widgets.rs

**Files:**
- Modify: `src/tui/widgets.rs`

- [ ] **Step 1: Add the help_bar function**

```rust
// Add to src/tui/widgets.rs after smooth_bar()

/// Shared help bar from key-label pairs
pub fn help_bar(bindings: &[(&str, &str)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![Span::styled("   ", Style::default())];
    for (key, label) in bindings {
        spans.push(Span::styled(key.to_string(), Style::default().fg(ACCENT)));
        spans.push(Span::styled(format!(" {}  ", label), Style::default().fg(FG_MUTED)));
    }
    Line::from(spans)
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets.rs
git commit -m "feat: add shared help_bar() widget"
```

### Task 4: Add `LayoutMode` to widgets.rs

**Files:**
- Modify: `src/tui/widgets.rs`

- [ ] **Step 1: Add LayoutMode enum**

```rust
// Add to src/tui/widgets.rs after the color constants (after line 10)

pub enum LayoutMode {
    Compact,   // < 100 cols: single column, sparklines hidden
    Standard,  // 100-139: 2-column summary
    Wide,      // >= 140: full 2-column with expanded bars
}

impl LayoutMode {
    pub fn from_width(w: u16) -> Self {
        match w {
            0..=99 => Self::Compact,
            100..=139 => Self::Standard,
            _ => Self::Wide,
        }
    }

    pub fn bar_width(&self) -> usize {
        match self {
            Self::Compact => 15,
            Self::Standard => 20,
            Self::Wide => 25,
        }
    }
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets.rs
git commit -m "feat: add LayoutMode for responsive terminal layouts"
```

### Task 5: Add `streak_days()` and `sessions_per_day()` to Store

**Files:**
- Modify: `src/store.rs`

- [ ] **Step 1: Add streak_days method**

Add after `avg_session_cost_historical()` (after line 714):

```rust
    /// Count consecutive days (ending today or yesterday) with at least one session
    pub fn streak_days(&self) -> usize {
        let mut dates: HashSet<NaiveDate> = HashSet::new();
        for r in &self.records {
            dates.insert(r.timestamp.date_naive());
        }
        let today = Utc::now().date_naive();
        let mut streak = 0usize;
        // Start from today, walk backwards
        let start = if dates.contains(&today) { today } else { today - Duration::days(1) };
        if !dates.contains(&start) { return 0; }
        let mut day = start;
        while dates.contains(&day) {
            streak += 1;
            day -= Duration::days(1);
        }
        streak
    }

    /// Sessions per day for the last N days (oldest first), for sparkline
    pub fn sessions_per_day(&self, days: usize) -> Vec<f64> {
        let day_data = self.by_day(days);
        let today = Utc::now().date_naive();
        let mut result = vec![0.0; days];
        for d in &day_data {
            let age = (today - d.date).num_days() as usize;
            if age < days {
                result[days - 1 - age] = d.session_count as f64;
            }
        }
        result
    }
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/store.rs
git commit -m "feat: add streak_days() and sessions_per_day() to Store"
```

### Task 6: Add `by_hour_all()` to Store for 24h activity chart

**Files:**
- Modify: `src/store.rs`

- [ ] **Step 1: Add by_hour_all method**

Add after `sessions_per_day()`:

```rust
    /// Token volume per hour of day (0-23), aggregated across all time
    pub fn by_hour_all(&self) -> [u64; 24] {
        let mut hours = [0u64; 24];
        for r in &self.records {
            let h = r.timestamp.hour() as usize;
            hours[h] += r.input_tokens + r.output_tokens;
        }
        hours
    }
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/store.rs
git commit -m "feat: add by_hour_all() for 24h activity heatmap"
```

---

## Chunk 2: Agent Spawn Detection in Parser

### Task 7: Add agent_spawns field to SessionMeta

**Files:**
- Modify: `src/parser/conversation.rs`

- [ ] **Step 1: Add agent_spawns to SessionMeta struct**

In `src/parser/conversation.rs`, add a new field to `SessionMeta` (after `tool_counts` on line 18):

```rust
    pub agent_spawns: usize,
```

- [ ] **Step 2: Count Agent tool_use calls in parse_session_meta**

In `parse_session_meta()`, after the line `extract_tool_names(&parsed.message, &mut tool_counts);` (line 96), the agent count is already captured in tool_counts. We just need to extract it.

After the `tools_used.sort_by(...)` block (after line 104), add:

```rust
    let agent_spawns = tool_counts.get("Agent").copied().unwrap_or(0);
```

Then update the `SessionMeta` construction (line 107-119) to include the new field:

```rust
    Ok(SessionMeta {
        session_id,
        project,
        file_path: path.to_string(),
        first_message,
        message_count: user_count + assistant_count,
        user_count,
        assistant_count,
        tools_used,
        tool_counts,
        agent_spawns,
        start_time: start_time.unwrap_or(now),
        end_time: end_time.unwrap_or(now),
    })
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly. The `agent_spawns` field is now populated from the existing `tool_counts` parsing.

- [ ] **Step 4: Commit**

```bash
git add src/parser/conversation.rs
git commit -m "feat: detect agent spawn count per session from tool_use"
```

---

## Chunk 3: Dashboard Redesign - Headline + Active Sessions

### Task 8: Replace cost headline with sessions/streak

**Files:**
- Modify: `src/tui/dashboard.rs`

- [ ] **Step 1: Rewrite the title section**

Replace the title rendering block in `render_main` (lines 160-175) with:

```rust
    // ── Title ──
    let today_agg = store.today();
    let week_agg = store.this_week();
    let streak = store.streak_days();

    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("   usagetracker", Style::default().fg(ACCENT).bold()),
            Span::styled(
                format!("{}sessions: {} today / {} this week   streak: {}d",
                    " ".repeat((w as usize).saturating_sub(65)),
                    today_agg.session_count,
                    week_agg.session_count,
                    streak,
                ),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ]);
    frame.render_widget(title, chunks[0]);
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. The `cost_rate` and `today_cost` variables are no longer used here. If they were used elsewhere in `render_main`, keep them; otherwise remove the dead variables.

- [ ] **Step 3: Commit**

```bash
git add src/tui/dashboard.rs
git commit -m "feat: replace cost headline with sessions/streak on dashboard"
```

### Task 9: Add agent spawn count to active session display

**Files:**
- Modify: `src/tui/dashboard.rs`

- [ ] **Step 1: Add agents indicator to context bar line**

In `render_main`, in the active sessions loop, modify the context bar line (the `Line::from(vec![...])` block at lines 231-251). Add the agent count after the compaction count:

Replace the block starting at line 231 with:

```rust
            // Line 2: context bar + agents
            let (bar_f, bar_e) = smooth_bar(ctx_pct, 100.0, bar_w);
            let mut ctx_spans = vec![
                Span::styled("     ctx ", Style::default().fg(FG_FAINT)),
                Span::styled(bar_f, Style::default().fg(bar_color)),
                Span::styled(bar_e, Style::default().fg(FG_FAINT)),
                Span::styled(
                    format!(" {:.0}%  {}  {:.1}x",
                        ctx_pct, compact(analysis.context_current), analysis.context_growth),
                    Style::default().fg(FG_MUTED),
                ),
                Span::styled(
                    format!("  cache {:.0}%", analysis.cache_hit_rate * 100.0),
                    Style::default().fg(FG_FAINT),
                ),
            ];
            if analysis.compaction_count > 0 {
                ctx_spans.push(Span::styled(
                    format!("  {} compactions", analysis.compaction_count),
                    Style::default().fg(FG_FAINT),
                ));
            }
            if meta.agent_spawns > 0 {
                ctx_spans.push(Span::styled(
                    format!("  {} agents", meta.agent_spawns),
                    Style::default().fg(YELLOW),
                ));
            }
            lines.push(Line::from(ctx_spans));
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. Agent spawn count now shows in yellow on active sessions.

- [ ] **Step 3: Commit**

```bash
git add src/tui/dashboard.rs
git commit -m "feat: show agent spawn count on active sessions"
```

### Task 10: Remove `[focused]` labels from dashboard

**Files:**
- Modify: `src/tui/dashboard.rs`

- [ ] **Step 1: Remove focused indicator from active sessions header**

In `render_main`, remove the `if in_active_zone` block from the LIVE header (lines 188-192). Replace with just closing the vec:

```rust
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("   LIVE", Style::default().fg(Color::Rgb(120, 190, 120)).bold()),
                Span::styled(
                    format!("  {} active session{}", active_count, if active_count > 1 { "s" } else { "" }),
                    Style::default().fg(FG_MUTED),
                ),
            ]),
        ];
```

- [ ] **Step 2: Remove focused indicator from projects header**

In the projects header section (lines 309-321), remove the `[focused]` conditional. Simplify to:

```rust
    let proj_header = Line::from(vec![
        Span::styled("   projects", Style::default().fg(ACCENT)),
        Span::styled(
            format!("{}tokens       cost   sessions    last",
                " ".repeat((w as usize).saturating_sub(62).max(2))),
            Style::default().fg(FG_MUTED),
        ),
    ]);
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. The cursor `>` already indicates focus.

- [ ] **Step 4: Commit**

```bash
git add src/tui/dashboard.rs
git commit -m "refactor: remove redundant [focused] labels from dashboard"
```

---

## Chunk 4: Dashboard 2-Column Summary

### Task 11: Replace period summary with 2-column layout

**Files:**
- Modify: `src/tui/dashboard.rs`

This is the biggest change. The period summary (today/yesterday/this week/all time) becomes a left column, and a 7-day mini trend chart goes in the right column.

- [ ] **Step 1: Increase the summary height constraint**

In `render_main`, change the `Constraint::Length(5)` for the summary chunk (line 150) to `Constraint::Length(6)` to accommodate the activity sparkline:

```rust
            Constraint::Length(6),              // summary (2-col: periods + 7d trend)
```

- [ ] **Step 2: Add the 2-column split and render**

Replace the entire period summary rendering block (from `// -- Today + Period Summary` through `frame.render_widget(Paragraph::new(period), chunks[3]);` which is lines 270-304) with:

```rust
    // ── Summary: 2-column layout ──
    let mode = LayoutMode::from_width(w);
    let today = store.today();
    let yesterday = store.yesterday();
    let week = store.this_week();

    match mode {
        LayoutMode::Compact => {
            // Single column fallback
            let period = vec![
                period_line("   today      ", &today, true, String::new(), FG_FAINT),
                period_line("   yesterday  ", &yesterday, false, String::new(), FG_FAINT),
                period_line("   this week  ", &week, false, String::new(), FG_FAINT),
            ];
            frame.render_widget(Paragraph::new(period), chunks[3]);
        }
        _ => {
            let summary_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(55),
                    Constraint::Percentage(45),
                ])
                .split(chunks[3]);

            // Left: period lines + budget gauge
            let (budget_str, budget_pct) = if let Some(budget) = config.budget_daily {
                let p = today.cost / budget * 100.0;
                (format!("  {:.0}% of daily budget", p), Some(p))
            } else if let Some(budget) = config.budget_weekly {
                let p = week.cost / budget * 100.0;
                (format!("  {:.0}% of weekly budget", p), Some(p))
            } else {
                (String::new(), None)
            };

            let budget_color = match budget_pct {
                Some(p) if p > 90.0 => RED,
                Some(p) if p > 70.0 => YELLOW,
                _ => FG_FAINT,
            };

            let left = vec![
                period_line("   today      ", &today, true, String::new(), FG_FAINT),
                period_line("   yesterday  ", &yesterday, false, String::new(), FG_FAINT),
                period_line("   this week  ", &week, false, String::new(), FG_FAINT),
                if budget_pct.is_some() {
                    let bp = budget_pct.unwrap_or(0.0);
                    let bw = 15usize;
                    let (bf, be) = smooth_bar(bp, 100.0, bw);
                    Line::from(vec![
                        Span::styled("   budget ", Style::default().fg(FG_MUTED)),
                        Span::styled(bf, Style::default().fg(budget_color)),
                        Span::styled(be, Style::default().fg(FG_FAINT)),
                        Span::styled(format!(" {:.0}%", bp), Style::default().fg(budget_color)),
                        Span::styled(budget_str, Style::default().fg(FG_FAINT)),
                    ])
                } else {
                    Line::from(Span::raw(""))
                },
            ];
            frame.render_widget(Paragraph::new(left), summary_cols[0]);

            // Right: 7-day trend mini chart
            let days_data = store.by_day(7);
            let sessions_spark = store.sessions_per_day(7);
            let spark_str = spark(&sessions_spark);

            let week_sessions: usize = days_data.iter().map(|d| d.session_count).sum();
            let mut right_lines = vec![
                Line::from(vec![
                    Span::styled("  7d ", Style::default().fg(FG_FAINT)),
                    Span::styled(spark_str, Style::default().fg(ACCENT)),
                    Span::styled(format!("  {} sessions", week_sessions), Style::default().fg(FG_MUTED)),
                ]),
            ];

            let max_tokens = days_data.iter()
                .map(|d| d.input_tokens + d.output_tokens + d.cache_creation_tokens + d.cache_read_tokens)
                .max()
                .unwrap_or(1);
            let mini_bar_w = 10usize;

            for day in days_data.iter().take(5).rev() {
                let total = day.input_tokens + day.output_tokens + day.cache_creation_tokens + day.cache_read_tokens;
                let weekday = day.date.format("%a").to_string();
                let (bf, be) = smooth_bar(total as f64, max_tokens as f64, mini_bar_w);
                right_lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", weekday), Style::default().fg(FG_FAINT)),
                    Span::styled(bf, Style::default().fg(ACCENT)),
                    Span::styled(be, Style::default().fg(FG_FAINT)),
                    Span::styled(format!(" {}  {}s", compact(total), day.session_count), Style::default().fg(FG_MUTED)),
                ]));
            }

            frame.render_widget(Paragraph::new(right_lines), summary_cols[1]);
        }
    }
```

- [ ] **Step 3: Remove unused budget variables**

The old `budget_str`, `budget_pct`, and `budget_color` variables that were defined before the period rendering block (lines 276-290) should be removed since the new code defines them inside the match arm.

- [ ] **Step 4: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. Dashboard now has 2-column summary in Standard/Wide mode.

- [ ] **Step 5: Commit**

```bash
git add src/tui/dashboard.rs
git commit -m "feat: 2-column dashboard summary with 7-day trend mini chart"
```

### Task 12: Add cost-per-turn sparkline to detail view

**Files:**
- Modify: `src/tui/dashboard.rs`

- [ ] **Step 1: Add sparkline after the context timeline**

In `render_detail`, after the timeline summary line (after the `if total_turns > shown` block, around line 549), add a cost-per-turn sparkline:

```rust
    // Cost per turn sparkline
    let costs: Vec<f64> = turns.iter().map(|t| t.cost).collect();
    if !costs.is_empty() {
        let (peak_idx, peak_cost) = costs.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, &c)| (i, c))
            .unwrap_or((0, 0.0));
        let cost_spark = spark(&costs);

        timeline_lines.push(Line::from(Span::raw("")));
        timeline_lines.push(Line::from(vec![
            Span::styled("   cost/turn: ", Style::default().fg(FG_FAINT)),
            Span::styled(cost_spark, Style::default().fg(ACCENT)),
            Span::styled(
                format!("  peak {} at turn {}", pricing::format_cost(peak_cost), peak_idx + 1),
                Style::default().fg(FG_MUTED),
            ),
        ]));
    }
```

- [ ] **Step 2: Add the pricing import if not present**

At the top of `dashboard.rs`, verify `use crate::pricing;` is already imported (it is, on line 2).

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. Detail view now shows a cost-per-turn sparkline at the bottom.

- [ ] **Step 4: Commit**

```bash
git add src/tui/dashboard.rs
git commit -m "feat: cost-per-turn sparkline in session detail view"
```

### Task 13: Use shared help_bar() in dashboard

**Files:**
- Modify: `src/tui/dashboard.rs`

- [ ] **Step 1: Replace help bar in render_main**

Replace the help bar block (lines 370-386) with:

```rust
    // ── Help bar ──
    let help = help_bar(&[
        ("\u{2191}\u{2193}", "navigate"),
        ("tab", "switch"),
        ("enter", "detail"),
        ("d", "daily"),
        ("t", "trends"),
        ("s", "sessions"),
        ("q", "quit"),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[8]);
```

- [ ] **Step 2: Replace help bar in render_detail**

Replace the help bar block in render_detail (lines 556-562) with:

```rust
    let help = help_bar(&[("esc", "back"), ("q", "quit")]);
    frame.render_widget(Paragraph::new(help), chunks[4]);
```

- [ ] **Step 3: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src/tui/dashboard.rs
git commit -m "refactor: use shared help_bar() in dashboard"
```

---

## Chunk 5: Insights View Overhaul

### Task 14: Replace unit economics with efficiency bars

**Files:**
- Modify: `src/tui/insights.rs`

- [ ] **Step 1: Move grade_color functions to widgets.rs**

Move `grade_color()` and `grade_color_inverse()` from insights.rs (lines 232-252) to widgets.rs, making them `pub`. Remove from insights.rs.

Add to `src/tui/widgets.rs`:

```rust
/// Green for good (higher is better)
pub fn grade_color(value: f64, low: f64, high: f64) -> Color {
    if value >= high {
        Color::Rgb(120, 190, 120)
    } else if value >= low {
        YELLOW
    } else {
        RED
    }
}

/// Red for bad (higher is worse)
pub fn grade_color_inverse(value: f64, warn: f64, crit: f64) -> Color {
    if value >= crit {
        RED
    } else if value >= warn {
        YELLOW
    } else {
        Color::Rgb(120, 190, 120)
    }
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. insights.rs resolves both functions via `use super::widgets::*`.

- [ ] **Step 3: Commit**

```bash
git add src/tui/widgets.rs src/tui/insights.rs
git commit -m "refactor: move grade_color functions to widgets for reuse"
```

### Task 15: Rewrite insights efficiency section with visual bars

**Files:**
- Modify: `src/tui/insights.rs`

- [ ] **Step 1: Replace the unit economics block**

Replace the econ section (lines 70-118 in insights.rs) with efficiency bars:

```rust
    // ── Efficiency bars ──
    let bar_w = 20usize;
    let cache_label = if cache_hit > 0.6 { "strong" } else if cache_hit > 0.3 { "fair" } else { "low" };
    let cache_c = grade_color(cache_hit, config.cache_alert_ratio, config.cache_alert_ratio * 2.0);
    let (cb_f, cb_e) = smooth_bar(cache_hit, 1.0, bar_w);

    let out_label = if out_in > 0.3 { "strong" } else if out_in > 0.1 { "fair" } else { "low" };
    let out_c = grade_color(out_in, 0.1, 0.3);
    let (ob_f, ob_e) = smooth_bar(out_in, 1.0, bar_w);

    let low_output_sessions = insights.sessions.iter()
        .filter(|s| s.total_input > 0 && (s.total_output as f64 / s.total_input as f64) < 0.05)
        .count();
    let waste_pct = if !insights.sessions.is_empty() {
        low_output_sessions as f64 / insights.sessions.len() as f64 * 100.0
    } else { 0.0 };
    let waste_label = if waste_pct < 15.0 { "ok" } else if waste_pct < 30.0 { "some" } else { "high" };
    let waste_c = grade_color_inverse(waste_pct, 15.0, 30.0);
    let (wb_f, wb_e) = smooth_bar(waste_pct, 100.0, bar_w);

    let econ = vec![
        Line::from(vec![
            Span::styled("   cache hit    ", Style::default().fg(FG_MUTED)),
            Span::styled(cb_f, Style::default().fg(cache_c)),
            Span::styled(cb_e, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:.0}%", cache_hit * 100.0), Style::default().fg(cache_c).bold()),
            Span::styled(format!("  {}", cache_label), Style::default().fg(cache_c)),
        ]),
        Line::from(vec![
            Span::styled("   output/input ", Style::default().fg(FG_MUTED)),
            Span::styled(ob_f, Style::default().fg(out_c)),
            Span::styled(ob_e, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:.0}%", out_in * 100.0), Style::default().fg(out_c).bold()),
            Span::styled(format!("  {}", out_label), Style::default().fg(out_c)),
        ]),
        Line::from(vec![
            Span::styled("   waste sess.  ", Style::default().fg(FG_MUTED)),
            Span::styled(wb_f, Style::default().fg(waste_c)),
            Span::styled(wb_e, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:.0}%", waste_pct), Style::default().fg(waste_c).bold()),
            Span::styled(format!("  {}", waste_label), Style::default().fg(waste_c)),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("   avg depth    ", Style::default().fg(FG_MUTED)),
            Span::styled(format!("{:.1} msgs/session", insights.avg_session_depth), Style::default().fg(FG)),
            Span::styled("      cost/session  ", Style::default().fg(FG_MUTED)),
            Span::styled(
                format!("{} (API eq.)", pricing::format_cost(insights.avg_cost_per_session)),
                Style::default().fg(FG_FAINT),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(econ), chunks[1]);
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. Efficiency section now has visual bars with color-coded labels.

- [ ] **Step 3: Commit**

```bash
git add src/tui/insights.rs
git commit -m "feat: efficiency bars with visual encoding in insights view"
```

### Task 16: Add 24h activity chart to insights

**Files:**
- Modify: `src/tui/insights.rs`

- [ ] **Step 1: Adjust layout constraints to fit the activity chart**

Replace the layout constraints in insights.rs (lines 13-25) with:

```rust
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),   // title + sparkline
            Constraint::Length(6),   // efficiency bars
            Constraint::Length(1),   // divider
            Constraint::Length(3),   // 24h activity chart
            Constraint::Length(1),   // divider
            Constraint::Min(4),     // heaviest sessions
            Constraint::Length(1),   // divider
            Constraint::Length(1),   // help
        ])
        .split(area);
```

- [ ] **Step 2: Add the 24h activity chart rendering**

After the first divider render (which was `frame.render_widget(Paragraph::new(divider(w)), chunks[2]);`), add:

```rust
    frame.render_widget(Paragraph::new(divider(w)), chunks[2]);

    // ── 24h Activity ──
    let hours = store.by_hour_all();
    let max_hour = hours.iter().copied().max().unwrap_or(1);
    let hour_values: Vec<f64> = hours.iter().map(|&h| h as f64).collect();
    let hour_spark = spark(&hour_values);

    let hour_labels: String = (0..24).map(|h| format!("{:>2}", h)).collect::<Vec<_>>().join("");
    // Only show label row if terminal is wide enough
    let activity_lines = if w >= 80 {
        vec![
            Line::from(vec![
                Span::styled("   activity by hour  ", Style::default().fg(FG_MUTED)),
                Span::styled(hour_spark, Style::default().fg(ACCENT)),
            ]),
            Line::from(vec![
                Span::styled("                     ", Style::default().fg(FG_FAINT)),
                Span::styled(
                    (0..24).map(|h| {
                        if h % 6 == 0 { format!("{:<4}", h) } else { "    ".to_string() }
                    }).collect::<Vec<_>>().join("")[..48.min(hour_labels.len())].to_string(),
                    Style::default().fg(FG_FAINT),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("   activity  ", Style::default().fg(FG_MUTED)),
                Span::styled(hour_spark, Style::default().fg(ACCENT)),
            ]),
        ]
    };
    frame.render_widget(Paragraph::new(activity_lines), chunks[3]);
    frame.render_widget(Paragraph::new(divider(w)), chunks[4]);
```

- [ ] **Step 3: Update remaining chunk indices**

The old model ROI section was at `chunks[3]`/`chunks[4]`. With the new layout, the heaviest sessions section is now at `chunks[5]`. Update all subsequent `chunks[N]` references:
- Heaviest sessions: `chunks[5]`
- Divider after sessions: `chunks[6]`
- Help bar: `chunks[7]`

- [ ] **Step 4: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. Insights now shows a 24h sparkline activity chart.

- [ ] **Step 5: Commit**

```bash
git add src/tui/insights.rs
git commit -m "feat: 24h activity sparkline chart in insights view"
```

### Task 17: Rename "costliest sessions" to "heaviest sessions", sort by message_count

**Files:**
- Modify: `src/tui/insights.rs`

- [ ] **Step 0: Add Clone derive to SessionInsight in store.rs**

In `src/store.rs`, change the derive on `SessionInsight` (line 56-57) from `#[derive(Debug)]` to `#[derive(Debug, Clone)]`. This is needed because we clone the sessions list to re-sort it.

- [ ] **Step 1: Change header text**

Find the "costliest sessions" header text and replace:

```rust
            Span::styled("   heaviest sessions", Style::default().fg(ACCENT)),
```

- [ ] **Step 2: Sort sessions by message_count instead of cost**

The sessions list comes from `insights.sessions` which is pre-sorted by cost in `store.rs:451`. We need to re-sort locally:

Before the sessions rendering loop, add:

```rust
    let mut sorted_sessions = insights.sessions.clone();
    sorted_sessions.sort_by(|a, b| b.message_count.cmp(&a.message_count));
```

Then iterate `sorted_sessions` instead of `insights.sessions`.

- [ ] **Step 3: Replace cost column header with "turns" and remove cost from the display**

Update the column headers to:
```rust
            Span::styled(
                format!("{}model    msgs   out/in   cache%    turns",
                    " ".repeat((w as usize).saturating_sub(72).max(2))),
                Style::default().fg(FG_MUTED),
            ),
```

In the per-session row, replace the cost span with message count:

```rust
            Span::styled(format!("{:>9}", s.message_count), Style::default().fg(FG_MUTED)),
```

- [ ] **Step 4: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. Sessions now sorted by weight (message count), not cost.

- [ ] **Step 5: Commit**

```bash
git add src/tui/insights.rs
git commit -m "feat: rename costliest to heaviest sessions, sort by message count"
```

### Task 18: Remove model ROI section from insights (demote to models view)

**Files:**
- Modify: `src/tui/insights.rs`

The model ROI table was previously occupying chunks[3]/chunks[4]. With the new layout, the 24h activity chart replaces it. The model breakdown remains in the dedicated Models view (`m` key). Remove the model ROI rendering code from insights if it still exists after the layout change.

- [ ] **Step 1: Verify model ROI code is removed**

After the layout restructure in Task 16, ensure no model ROI rendering code remains. If any dead code exists from the old `roi_lines` block, delete it.

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly with no dead code warnings.

- [ ] **Step 3: Commit (if changes needed)**

```bash
git add src/tui/insights.rs
git commit -m "refactor: remove model ROI from insights, available in models view"
```

### Task 19: Use shared help_bar() in insights

**Files:**
- Modify: `src/tui/insights.rs`

- [ ] **Step 1: Replace help bar**

Replace the help bar block in insights.rs with:

```rust
    let help = help_bar(&[("esc", "back"), ("q", "quit")]);
    frame.render_widget(Paragraph::new(help), chunks[7]);
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/tui/insights.rs
git commit -m "refactor: use shared help_bar() in insights view"
```

---

## Chunk 6: Final Polish

### Task 20: Update title sparkline to sessions instead of cost

**Files:**
- Modify: `src/tui/insights.rs`

- [ ] **Step 1: Change the title sparkline data**

Replace the title section (the `sparkline_str` and total cost) with sessions-based data:

```rust
    // ── Title ──
    let sessions_spark_data = store.sessions_per_day(config.sparkline_days);
    let sparkline_str = spark(&sessions_spark_data);
    let total_sessions_period: f64 = sessions_spark_data.iter().sum();
    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("   insights", Style::default().fg(ACCENT).bold()),
            Span::styled(
                format!("{}{}d activity  {}  {:.0} sessions",
                    " ".repeat((w as usize).saturating_sub(58)),
                    config.sparkline_days,
                    sparkline_str,
                    total_sessions_period,
                ),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ]);
    frame.render_widget(title, chunks[0]);
```

- [ ] **Step 2: Build and verify**

Run: `cargo build 2>&1`
Expected: Compiles. Title now shows session activity, not cost.

- [ ] **Step 3: Commit**

```bash
git add src/tui/insights.rs
git commit -m "feat: insights title sparkline shows sessions, not cost"
```

### Task 21: Full build + manual verification

- [ ] **Step 1: Clean build**

Run: `cargo build --release 2>&1`
Expected: Compiles with zero warnings.

- [ ] **Step 2: Run the TUI and verify visually**

Run: `cargo run --release`
Verify:
- Dashboard shows sessions/streak in title (not cost/hr)
- Active sessions show agent spawn count when present
- 2-column layout appears at >= 100 cols (left: periods, right: 7d trend)
- Detail view has cost-per-turn sparkline
- Insights shows efficiency bars, 24h activity, heaviest sessions
- All keybinds still work (d, t, m, i, s, tab, enter, esc, q)

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "feat: story-driven dashboard with visual graphs and subscription framing"
```
