pub mod tools;

use crate::config::Config;
use crate::pricing;
use crate::store::Store;
use tools::{SessionIdInput, ListSessionsInput, SearchSessionsInput};

use rmcp::{
    ErrorData,
    handler::server::tool::ToolRouter,
    model::*,
    tool, tool_handler, tool_router,
    handler::server::wrapper::Parameters,
    ServerHandler,
};

#[derive(Clone)]
pub struct UsageServer {
    store: Store,
    #[allow(dead_code)]
    config: Config,
    tool_router: ToolRouter<UsageServer>,
}

#[tool_router]
impl UsageServer {
    pub fn new(store: Store, config: Config) -> Self {
        Self {
            store,
            config,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get current session health metrics: context size, cost, cache hit rate, efficiency grade, compaction count. Use this to understand how the current conversation is performing.")]
    async fn session_health(
        &self,
        Parameters(input): Parameters<SessionIdInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let sid = input.session_id
            .or_else(|| self.store.most_recent_session_id())
            .ok_or_else(|| ErrorData::internal_error("No sessions found", None))?;

        let analysis = self.store.analyze_session(&sid)
            .ok_or_else(|| ErrorData::internal_error("Session not found", None))?;

        let grade = compute_grade(&analysis);

        let response = format!(
            "Session Health: {grade}\n\
             Context: {ctx} tokens (started at {init}, peak {peak}, {growth:.1}x growth)\n\
             Messages: {msgs} ({since_compact} since last compaction)\n\
             Cost: {cost} (cache read {cr}, cache write {cw}, output {out})\n\
             Cache hit rate: {cache:.0}%\n\
             Output efficiency: {eff:.3}% (output tokens / context tokens)\n\
             Compactions: {compactions}",
            grade = grade,
            ctx = analysis.context_current,
            init = analysis.context_initial,
            peak = analysis.context_peak,
            growth = analysis.context_growth,
            msgs = analysis.message_count,
            since_compact = analysis.messages_since_compaction,
            cost = pricing::format_cost(analysis.total_cost),
            cr = pricing::format_cost(analysis.cost_breakdown.cache_read),
            cw = pricing::format_cost(analysis.cost_breakdown.cache_write),
            out = pricing::format_cost(analysis.cost_breakdown.output),
            cache = analysis.cache_hit_rate * 100.0,
            eff = analysis.output_efficiency,
            compactions = analysis.compaction_count,
        );

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "Get detailed cost breakdown for a session: cost by token type, context growth premium (extra cost from growing context), cost per 1K output tokens, comparison to historical average.")]
    async fn session_cost(
        &self,
        Parameters(input): Parameters<SessionIdInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let sid = input.session_id
            .or_else(|| self.store.most_recent_session_id())
            .ok_or_else(|| ErrorData::internal_error("No sessions found", None))?;

        let analysis = self.store.analyze_session(&sid)
            .ok_or_else(|| ErrorData::internal_error("Session not found", None))?;

        let avg = self.store.avg_session_cost_historical();

        let comparison = if avg > 0.0 && analysis.total_cost > avg * 1.5 {
            format!("{:.0}% above average", (analysis.total_cost / avg - 1.0) * 100.0)
        } else if avg > 0.0 && analysis.total_cost < avg * 0.5 {
            format!("{:.0}% below average", (1.0 - analysis.total_cost / avg) * 100.0)
        } else {
            "near average".to_string()
        };

        let response = format!(
            "Session Cost: {total}\n\
             \n\
             Breakdown:\n\
             - Cache read:  {cr} ({cr_pct:.0}%)\n\
             - Cache write: {cw} ({cw_pct:.0}%)\n\
             - Output:      {out} ({out_pct:.0}%)\n\
             - Input:       {inp} ({inp_pct:.0}%)\n\
             \n\
             Context growth premium: {premium}\n\
             (Extra cost from context growing {growth:.1}x over the session.\n\
              If every message had the initial context size, this session would cost {savings} less.)\n\
             \n\
             Cost per 1K output tokens: {cpo}\n\
             vs historical average session: {avg}\n\
             This session is {comparison}.",
            total = pricing::format_cost(analysis.total_cost),
            cr = pricing::format_cost(analysis.cost_breakdown.cache_read),
            cr_pct = if analysis.total_cost > 0.0 { analysis.cost_breakdown.cache_read / analysis.total_cost * 100.0 } else { 0.0 },
            cw = pricing::format_cost(analysis.cost_breakdown.cache_write),
            cw_pct = if analysis.total_cost > 0.0 { analysis.cost_breakdown.cache_write / analysis.total_cost * 100.0 } else { 0.0 },
            out = pricing::format_cost(analysis.cost_breakdown.output),
            out_pct = if analysis.total_cost > 0.0 { analysis.cost_breakdown.output / analysis.total_cost * 100.0 } else { 0.0 },
            inp = pricing::format_cost(analysis.cost_breakdown.input),
            inp_pct = if analysis.total_cost > 0.0 { analysis.cost_breakdown.input / analysis.total_cost * 100.0 } else { 0.0 },
            premium = pricing::format_cost(analysis.context_growth_premium),
            growth = analysis.context_growth,
            savings = pricing::format_cost(analysis.context_growth_premium),
            cpo = pricing::format_cost(analysis.cost_per_1k_output),
            avg = pricing::format_cost(avg),
            comparison = comparison,
        );

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "Get a recommendation on whether to start a new session. Analyzes context growth, efficiency degradation, and projected cost savings from restarting.")]
    async fn should_restart(
        &self,
        Parameters(input): Parameters<SessionIdInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let sid = input.session_id
            .or_else(|| self.store.most_recent_session_id())
            .ok_or_else(|| ErrorData::internal_error("No sessions found", None))?;

        let analysis = self.store.analyze_session(&sid)
            .ok_or_else(|| ErrorData::internal_error("Session not found", None))?;

        let (recommendation, reasoning) = restart_recommendation(&analysis);

        let response = format!(
            "Recommendation: {rec}\n\
             \n\
             {reasoning}\n\
             \n\
             Current state:\n\
             - Context: {ctx} tokens ({growth:.1}x from start)\n\
             - Efficiency: {eff:.3}% output/context\n\
             - Messages since compaction: {since}\n\
             - Context growth premium so far: {premium}",
            rec = recommendation,
            reasoning = reasoning,
            ctx = analysis.context_current,
            growth = analysis.context_growth,
            eff = analysis.output_efficiency,
            since = analysis.messages_since_compaction,
            premium = pricing::format_cost(analysis.context_growth_premium),
        );

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "List recent coding sessions with metadata: project, topic, message count, duration, cost. Use this to find specific past conversations or understand recent work patterns.")]
    async fn list_sessions(
        &self,
        Parameters(input): Parameters<ListSessionsInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = input.limit.unwrap_or(10);
        let sessions = self.store.sessions_by_time();

        let filtered: Vec<_> = if let Some(ref proj) = input.project {
            let p = proj.to_lowercase();
            sessions.into_iter()
                .filter(|s| s.project.to_lowercase().contains(&p))
                .take(limit)
                .collect()
        } else {
            sessions.into_iter().take(limit).collect()
        };

        if filtered.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text("No sessions found.")]));
        }

        let mut lines = Vec::new();
        for s in &filtered {
            let cost = self.store.session_cost(&s.session_id);
            let dur = s.duration_minutes();
            let dur_str = if dur >= 60 {
                format!("{}h{:02}m", dur / 60, dur % 60)
            } else {
                format!("{}m", dur.max(1))
            };
            lines.push(format!(
                "{date}  {project:<20}  {msgs:>4} msgs  {dur:>6}  {cost:>8}  {topic}",
                date = s.start_time.format("%Y-%m-%d %H:%M"),
                project = s.project,
                msgs = s.user_count,
                dur = dur_str,
                cost = pricing::format_cost(cost),
                topic = s.first_message,
            ));
        }

        let response = format!(
            "{} sessions (showing {}):\n\n{}",
            self.store.sessions_by_time().len(),
            filtered.len(),
            lines.join("\n"),
        );

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }

    #[tool(description = "Search past sessions by keyword. Searches across session topics and project names. Returns matching sessions with metadata.")]
    async fn search_sessions(
        &self,
        Parameters(input): Parameters<SearchSessionsInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = input.limit.unwrap_or(10);
        let results = self.store.search_sessions(&input.query);

        if results.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                format!("No sessions found matching '{}'.", input.query)
            )]));
        }

        let mut lines = Vec::new();
        for s in results.iter().take(limit) {
            let cost = self.store.session_cost(&s.session_id);
            let dur = s.duration_minutes();
            let dur_str = if dur >= 60 {
                format!("{}h{:02}m", dur / 60, dur % 60)
            } else {
                format!("{}m", dur.max(1))
            };
            lines.push(format!(
                "{date}  {project:<20}  {msgs:>4} msgs  {dur:>6}  {cost:>8}  {topic}",
                date = s.start_time.format("%Y-%m-%d %H:%M"),
                project = s.project,
                msgs = s.user_count,
                dur = dur_str,
                cost = pricing::format_cost(cost),
                topic = s.first_message,
            ));
        }

        let response = format!(
            "Found {} sessions matching '{}' (showing {}):\n\n{}",
            results.len(),
            input.query,
            lines.len().min(limit),
            lines.join("\n"),
        );

        Ok(CallToolResult::success(vec![Content::text(response)]))
    }
}

#[tool_handler]
impl ServerHandler for UsageServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::new(
            "usagetracker",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_instructions(
            "Session analyst for Claude Code. Provides real-time session health, \
             cost analysis, and restart recommendations based on context growth \
             and efficiency metrics."
        )
    }
}

fn compute_grade(analysis: &crate::store::SessionAnalysis) -> &'static str {
    let mut score = 100i32;

    // Penalize context growth
    if analysis.context_growth > 8.0 { score -= 30; }
    else if analysis.context_growth > 5.0 { score -= 20; }
    else if analysis.context_growth > 3.0 { score -= 10; }

    // Penalize low efficiency
    if analysis.output_efficiency < 0.1 { score -= 30; }
    else if analysis.output_efficiency < 0.2 { score -= 15; }

    // Penalize high cost per output
    if analysis.cost_per_1k_output > 1.0 { score -= 20; }
    else if analysis.cost_per_1k_output > 0.5 { score -= 10; }

    // Bonus for compactions (system is managing context)
    if analysis.compaction_count > 0 { score += 5; }

    match score {
        90..=200 => "A (healthy)",
        75..=89 => "B (good)",
        60..=74 => "C (fair, consider restarting soon)",
        40..=59 => "D (degraded, restart recommended)",
        _ => "F (poor, restart strongly recommended)",
    }
}

fn restart_recommendation(analysis: &crate::store::SessionAnalysis) -> (&'static str, String) {
    // Near compaction ceiling (167K is typical Claude Code limit)
    if analysis.context_current > 150_000 {
        return ("YES", format!(
            "Context is at {} tokens, approaching the ~167K compaction ceiling. \
             A compaction will happen soon, losing conversation history. \
             Better to start fresh intentionally.",
            analysis.context_current
        ));
    }

    // High context growth with many messages since compaction
    if analysis.context_growth > 6.0 && analysis.messages_since_compaction > 100 {
        return ("YES", format!(
            "Context has grown {:.1}x with {} messages since last compaction. \
             Efficiency is {:.3}%. Starting fresh would reduce cost per message significantly. \
             Estimated savings: {} if remaining work continues at current rate.",
            analysis.context_growth,
            analysis.messages_since_compaction,
            analysis.output_efficiency,
            pricing::format_cost(analysis.context_growth_premium * 0.3),
        ));
    }

    // Moderate growth
    if analysis.context_growth > 4.0 {
        return ("SOON", format!(
            "Context has grown {:.1}x. Not critical yet, but efficiency is declining. \
             Consider restarting after completing your current task.",
            analysis.context_growth,
        ));
    }

    // Healthy
    ("NO", format!(
        "Session looks healthy. Context at {} tokens ({:.1}x growth), \
         efficiency at {:.3}%. No need to restart.",
        analysis.context_current,
        analysis.context_growth,
        analysis.output_efficiency,
    ))
}
