use crate::config::Config;
use crate::parser::Source;
use crate::pricing;
use crate::store::{Store, SessionTimeline};
use crate::store::analysis;
use super::widgets::*;
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct DashboardState {
    pub detail: Option<SessionDetailView>,
}

pub struct SessionDetailView {
    pub session_id: String,
    pub timeline: SessionTimeline,
}

impl DashboardState {
    pub fn back(&mut self) -> bool {
        if self.detail.is_some() { self.detail = None; true } else { false }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Session detail view: health panel + context growth chart
//  Triggered from Browser via Enter on a session.
// ════════════════════════════════════════════════════════════════════════

pub fn render_detail(
    frame: &mut ratatui::Frame,
    store: &Store,
    config: &Config,
    state: &mut DashboardState,
    live_sessions: &HashMap<String, bool>,
) {
    let area = frame.area();
    let w = area.width;
    let detail = match &state.detail {
        Some(d) => d,
        None => return,
    };

    let sessions = store.sessions_by_time();
    let meta = sessions.iter().find(|s| s.session_id == detail.session_id);
    let analysis = store.analyze_session(&detail.session_id);
    let is_live = live_sessions.get(&detail.session_id).copied().unwrap_or(false);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),  // Layer 1: header
            Constraint::Length(1),  // blank
            Constraint::Length(3),  // Layer 2: health panel
            Constraint::Length(1),  // blank
            Constraint::Min(4),    // Layer 3: context growth chart
            Constraint::Length(1),  // help bar
        ])
        .split(area);

    // ── Layer 1: Header ──
    if let Some(meta) = meta {
        let cost = detail.timeline.total_cost;
        let dur = detail.timeline.duration_minutes;
        let dur_str = if dur >= 60 { format!("{}h{:02}m", dur / 60, dur % 60) } else { format!("{}m", dur.max(1)) };

        let ceiling = store.session_meta(&detail.session_id).and_then(|m| m.context_token_limit);
        let health = if let Some(ref a) = analysis {
            let status = analysis::health_status(a, ceiling, is_live, config.context_warn_pct, config.context_danger_pct);
            (status.label(), health_color(&status))
        } else {
            ("", FG_FAINT)
        };

        let source_badge = match meta.source {
            Source::ClaudeCode => ("\u{25cf} CC", ACCENT2),
            Source::Cursor => ("\u{25cf} Cu", BLUE),
        };

        let header = vec![
            Line::from(vec![
                Span::styled(format!("   {}", truncate(&meta.first_message, (w as usize).saturating_sub(10))),
                    Style::default().fg(FG).bold()),
            ]),
            Line::from(vec![
                Span::styled(format!("   {}", source_badge.0), Style::default().fg(source_badge.1)),
                Span::styled(format!("  {}", display_project_name(&meta.project)), Style::default().fg(ACCENT)),
                Span::styled(format!("  {}  {}  {}t", dur_str, pricing::format_cost(cost), meta.user_count),
                    Style::default().fg(FG_MUTED)),
                Span::styled(format!("  {}", health.0), Style::default().fg(health.1).bold()),
            ]),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        // ── Layer 2: Health panel ──
        let mut health_lines: Vec<Line> = Vec::new();

        if let Some(ref a) = analysis {
            let (ctx_pct, _) = if let Some(ceil) = ceiling {
                let pct = (a.context_current as f64 / ceil as f64 * 100.0).min(100.0);
                (pct, format!("{}/{}", compact(a.context_current), compact(ceil)))
            } else {
                let pct = if a.context_peak > 0 { (a.context_current as f64 / a.context_peak as f64 * 100.0).min(100.0) } else { 0.0 };
                (pct, compact(a.context_current))
            };

            let bar_w = (w as usize).saturating_sub(55).max(10);
            let (bf, be) = smooth_bar(ctx_pct, 100.0, bar_w);
            let color = ctx_color(ctx_pct);

            let (total_in, total_out) = store.session_tokens(&detail.session_id);
            let total_tokens = total_in + total_out;

            health_lines.push(Line::from(vec![
                Span::styled("   ctx  ", Style::default().fg(FG_FAINT)),
                Span::styled(bf, Style::default().fg(color)),
                Span::styled(be, Style::default().fg(FG_FAINT)),
                Span::styled(format!("  {:.0}%", ctx_pct), Style::default().fg(color).bold()),
                Span::styled(format!("   {} > {}", compact(a.context_initial), compact(a.context_current)),
                    Style::default().fg(FG_MUTED)),
                Span::styled(format!("   {:.1}x growth", a.context_growth), Style::default().fg(FG_FAINT)),
                Span::styled(format!("   cache {:.0}%", a.cache_hit_rate * 100.0), Style::default().fg(FG_FAINT)),
                Span::styled(format!("   {} total (in {} out {})", compact(total_tokens), compact(total_in), compact(total_out)),
                    Style::default().fg(FG_MUTED)),
            ]));

            let cb = &a.cost_breakdown;
            health_lines.push(Line::from(vec![
                Span::styled(format!("   cost out {}   in {}   cache-r {}   cache-w {}",
                    pricing::format_cost(cb.output), pricing::format_cost(cb.input),
                    pricing::format_cost(cb.cache_read), pricing::format_cost(cb.cache_write)),
                    Style::default().fg(FG_MUTED)),
            ]));

            let top_tools: Vec<String> = meta.tools_used.iter().take(8)
                .map(|t| { let c = meta.tool_counts.get(t).unwrap_or(&0); format!("{}({})", t, c) })
                .collect();
            if !top_tools.is_empty() {
                health_lines.push(Line::from(vec![
                    Span::styled("   tools ", Style::default().fg(FG_FAINT)),
                    Span::styled(top_tools.join("  "), Style::default().fg(FG_MUTED)),
                    if meta.agent_spawns > 0 {
                        Span::styled(format!("   {} agents spawned", meta.agent_spawns), Style::default().fg(PURPLE))
                    } else { Span::raw("") },
                ]));
            }
        }

        while health_lines.len() < 3 { health_lines.push(Line::from(Span::raw(""))); }
        frame.render_widget(Paragraph::new(health_lines), chunks[2]);
    }

    // ── Layer 3: Context growth chart ──
    let turns = &detail.timeline.turns;
    let bar_w = (w as usize).saturating_sub(40).max(10);
    let avg_cost = detail.timeline.avg_cost_per_turn;
    let spike_threshold = avg_cost * 2.5;

    let thresholds = [25.0, 50.0, 75.0, 85.0];
    let mut last_crossed: Option<usize> = None;
    let mut notable_indices: Vec<usize> = Vec::new();

    for (i, turn) in turns.iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == turns.len() - 1;
        let current_threshold = thresholds.iter().rposition(|&t| turn.context_pct >= t);
        let crossed_new = current_threshold != last_crossed;
        if crossed_new { last_crossed = current_threshold; }

        let is_notable = is_first || is_last || turn.is_compaction
            || (spike_threshold > 0.0 && turn.cost > spike_threshold) || crossed_new;
        if is_notable {
            notable_indices.push(i);
        }
    }

    let mut timeline_lines: Vec<Line> = Vec::new();
    timeline_lines.push(Line::from(Span::styled(
        "   CONTEXT GROWTH", Style::default().fg(ACCENT).bold(),
    )));

    let mut prev_time: Option<chrono::DateTime<chrono::Utc>> = None;
    for &idx in &notable_indices {
        let turn = &turns[idx];
        let is_first = idx == 0;
        let is_last = idx == turns.len() - 1;

        let delta_str = if is_first {
            "  0m".to_string()
        } else if let Some(prev) = prev_time {
            let delta = (turn.timestamp - prev).num_minutes();
            if delta >= 60 { format!("{:>3}h", delta / 60) } else { format!("{:>3}m", delta) }
        } else {
            "  0m".to_string()
        };
        prev_time = Some(turn.timestamp);

        let filled = ((turn.context_pct / 100.0) * bar_w as f64).round() as usize;
        let bar_filled: String = "\u{2588}".repeat(filled);
        let bar_empty: String = "\u{2591}".repeat(bar_w.saturating_sub(filled));
        let bar_color = ctx_color(turn.context_pct);

        let event_label = if is_first { "started".to_string() }
            else if turn.is_compaction { "\u{2193} compacted".to_string() }
            else if is_last { "current".to_string() }
            else if turn.context_pct > 85.0 { "\u{26a0} near limit".to_string() }
            else if spike_threshold > 0.0 && turn.cost > spike_threshold {
                format!("cost spike {}", pricing::format_cost(turn.cost))
            }
            else { String::new() };
        let event_color = if turn.is_compaction { YELLOW }
            else if turn.context_pct > 85.0 { RED }
            else if spike_threshold > 0.0 && turn.cost > spike_threshold { YELLOW }
            else if is_last { ACCENT }
            else { FG_FAINT };

        timeline_lines.push(Line::from(vec![
            Span::styled(format!("   {} ", delta_str), Style::default().fg(FG_FAINT)),
            Span::styled(bar_filled, Style::default().fg(bar_color)),
            Span::styled(bar_empty, Style::default().fg(FG_FAINT)),
            Span::styled(format!("  {:>5}", compact(turn.context_size)), Style::default().fg(FG_FAINT)),
            if !event_label.is_empty() {
                Span::styled(format!("   {}", event_label), Style::default().fg(event_color))
            } else { Span::raw("") },
        ]));
    }

    // Cost sparkline
    let costs: Vec<f64> = turns.iter().map(|t| t.cost).collect();
    if !costs.is_empty() {
        let (peak_idx, peak_cost) = costs.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, &c)| (i, c)).unwrap_or((0, 0.0));
        timeline_lines.push(Line::from(Span::raw("")));
        timeline_lines.push(Line::from(vec![
            Span::styled("   cost/turn ", Style::default().fg(FG_FAINT)),
            Span::styled(spark(&costs), Style::default().fg(ACCENT)),
            Span::styled(format!("   peak {} at turn {}", pricing::format_cost(peak_cost), peak_idx + 1), Style::default().fg(FG_MUTED)),
        ]));
    }

    // Activity strip
    if turns.len() >= 2 {
        let start = turns.first().unwrap().timestamp;
        let end = turns.last().unwrap().timestamp;
        let total_minutes = (end - start).num_minutes().max(1);
        let strip_w = (w as usize).saturating_sub(20).clamp(10, 40);

        let mut slots = vec![false; strip_w];
        for t in turns {
            let offset = (t.timestamp - start).num_minutes();
            let slot = ((offset as f64 / total_minutes as f64) * (strip_w - 1) as f64).round() as usize;
            if slot < strip_w { slots[slot] = true; }
        }

        let strip = density_strip(&slots);
        let start_str = start.format("%H:%M").to_string();
        let end_str = end.format("%H:%M").to_string();

        timeline_lines.push(Line::from(Span::raw("")));
        timeline_lines.push(Line::from(vec![
            Span::styled("     ", Style::default()),
            Span::styled(strip, Style::default().fg(FG_MUTED)),
            Span::styled("   activity pattern", Style::default().fg(FG_FAINT)),
        ]));
        timeline_lines.push(Line::from(vec![
            Span::styled(format!("     {}", start_str), Style::default().fg(FG_FAINT)),
            Span::styled(
                " ".repeat(strip_w.saturating_sub(start_str.len() + end_str.len()).max(1)),
                Style::default(),
            ),
            Span::styled(end_str, Style::default().fg(FG_FAINT)),
        ]));
    }

    frame.render_widget(Paragraph::new(timeline_lines), chunks[4]);

    // ── Help bar ──
    let help = help_bar(&[("esc", "back to browser"), ("q", "quit")]);
    frame.render_widget(Paragraph::new(help), chunks[5]);
}
