use crate::parser::{Source, UsageRecord};
use crate::parser::conversation::{SessionMeta, SessionStatus, SessionMode, CursorTodo};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

/// Parse Cursor sessions from state.vscdb into UsageRecords.
/// Each session becomes one UsageRecord with aggregated token counts from all assistant bubbles.
pub fn parse_cursor_db(db_path: &str) -> anyhow::Result<(Vec<UsageRecord>, Vec<SessionMeta>)> {
    let conn = Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let mut records = Vec::new();
    let mut metas = Vec::new();

    // Read all composerData entries
    let mut stmt = conn.prepare(
        "SELECT key, value FROM cursorDiskKV WHERE key LIKE 'composerData:%'"
    )?;
    let sessions: Vec<(String, String)> = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?.filter_map(|r| r.ok()).collect();

    for (key, value) in &sessions {
        let composer_id = key.strip_prefix("composerData:").unwrap_or(key);
        let data: serde_json::Value = match serde_json::from_str(value) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let created_at = data.get("createdAt").and_then(|v| v.as_i64()).unwrap_or(0);
        let last_updated = data.get("lastUpdatedAt").and_then(|v| v.as_i64()).unwrap_or(created_at);
        if created_at == 0 { continue; }

        let start_time = millis_to_datetime(created_at);
        let end_time = millis_to_datetime(last_updated);

        let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let status_str = data.get("status").and_then(|v| v.as_str()).unwrap_or("none");
        let model_name = data.get("modelConfig")
            .and_then(|mc| mc.get("modelName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let headers = data.get("fullConversationHeadersOnly")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        // Skip empty sessions
        if headers == 0 && status_str == "none" { continue; }

        // Parse Cursor-specific fields
        let cursor_status = match status_str {
            "completed" => SessionStatus::Completed,
            "aborted" => SessionStatus::Aborted,
            _ => SessionStatus::None,
        };

        let cursor_mode = match data.get("unifiedMode").and_then(|v| v.as_str()).unwrap_or("") {
            "agent" => SessionMode::Agent,
            "chat" => SessionMode::Chat,
            "plan" => SessionMode::Plan,
            _ => SessionMode::Chat,
        };

        let lines_added = data.get("totalLinesAdded").and_then(|v| v.as_u64());
        let lines_removed = data.get("totalLinesRemoved").and_then(|v| v.as_u64());
        let files_changed = data.get("filesChangedCount").and_then(|v| v.as_u64());

        let context_tokens_used = data.get("contextTokensUsed").and_then(|v| v.as_u64());
        let context_token_limit = data.get("contextTokenLimit").and_then(|v| v.as_u64());
        let context_usage_pct = data.get("contextUsagePercent").and_then(|v| v.as_f64());

        let subtitle = data.get("subtitle").and_then(|v| v.as_str()).map(String::from);
        let added_files_count = data.get("addedFiles").and_then(|v| v.as_u64());
        let removed_files_count = data.get("removedFiles").and_then(|v| v.as_u64());

        let is_agentic = data.get("isAgentic").and_then(|v| v.as_bool());
        let subagent_count = data.get("subagentComposerIds")
            .and_then(|v| v.as_array())
            .map(|a| a.len());

        let cursor_todos: Option<Vec<CursorTodo>> = data.get("todos")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().filter_map(|t| {
                    let content = t.get("content").and_then(|c| c.as_str())?.to_string();
                    let completed = t.get("status").and_then(|s| s.as_str()) == Some("completed");
                    Some(CursorTodo { content, completed })
                }).collect()
            });

        // Aggregate token counts from bubbles
        let (input_tokens, output_tokens, _bubble_count) = aggregate_bubbles(&conn, composer_id);

        // Only include sessions that have some activity
        if input_tokens == 0 && output_tokens == 0 && headers < 2 { continue; }

        let project = extract_cursor_project(db_path);

        // Create one UsageRecord per session (aggregated)
        records.push(UsageRecord {
            timestamp: start_time,
            session_id: composer_id.to_string(),
            project: project.clone(),
            model: model_name.clone(),
            source: Source::Cursor,
            input_tokens,
            output_tokens,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
        });

        // Count user vs assistant from headers
        let header_list = data.get("fullConversationHeadersOnly")
            .and_then(|v| v.as_array());
        let (user_count, assistant_count) = match header_list {
            Some(arr) => {
                let u = arr.iter().filter(|h| h.get("type").and_then(|t| t.as_i64()) == Some(1)).count();
                let a = arr.iter().filter(|h| h.get("type").and_then(|t| t.as_i64()) == Some(2)).count();
                (u, a)
            }
            None => (0, 0),
        };

        let first_message = if name.is_empty() {
            format!("Cursor session ({})", &composer_id[..8.min(composer_id.len())])
        } else {
            name
        };

        metas.push(SessionMeta {
            session_id: composer_id.to_string(),
            project,
            file_path: db_path.to_string(),
            first_message,
            source: Source::Cursor,
            message_count: user_count + assistant_count,
            user_count,
            assistant_count,
            tools_used: Vec::new(),
            tool_counts: HashMap::new(),
            agent_spawns: 0,
            start_time,
            end_time,
            cursor_status: Some(cursor_status),
            cursor_mode: Some(cursor_mode),
            lines_added,
            lines_removed,
            files_changed,
            context_tokens_used,
            context_token_limit,
            context_usage_pct,
            cursor_todos,
            is_agentic,
            subagent_count,
            parent_session_id: None,
            is_subagent: false,
            agent_type: None,
            cursor_subtitle: subtitle,
            cursor_model_name: Some(model_name.clone()),
            added_files: added_files_count,
            removed_files: removed_files_count,
        });
    }

    Ok((records, metas))
}

/// Sum inputTokens and outputTokens from all type=2 bubbles for a given composer session.
fn aggregate_bubbles(conn: &Connection, composer_id: &str) -> (u64, u64, usize) {
    let pattern = format!("bubbleId:{}:%", composer_id);
    let mut stmt = match conn.prepare("SELECT value FROM cursorDiskKV WHERE key LIKE ?1") {
        Ok(s) => s,
        Err(_) => return (0, 0, 0),
    };

    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut count = 0usize;

    let rows: Vec<String> = stmt.query_map([&pattern], |row| {
        row.get::<_, String>(0)
    }).map(|iter| iter.filter_map(|r| r.ok()).collect()).unwrap_or_default();

    for value in &rows {
        let data: serde_json::Value = match serde_json::from_str(value) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Only assistant bubbles (type=2) have token counts
        if data.get("type").and_then(|t| t.as_i64()) != Some(2) { continue; }

        if let Some(tc) = data.get("tokenCount") {
            let inp = tc.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
            let out = tc.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
            if inp > 0 || out > 0 {
                total_input += inp;
                total_output += out;
                count += 1;
            }
        }
    }

    (total_input, total_output, count)
}

/// Extract project name from Cursor workspace storage.
/// Falls back to "Cursor" if no workspace mapping found.
fn extract_cursor_project(db_path: &str) -> String {
    // Try to find workspace mapping via workspaceStorage
    // db_path is typically: ~/Library/Application Support/Cursor/User/globalStorage/state.vscdb
    let base = Path::new(db_path)
        .parent()          // globalStorage/
        .and_then(|p| p.parent())  // User/
        .map(|p| p.join("workspaceStorage"));

    if let Some(ws_dir) = base {
        if ws_dir.exists() {
            // Look through workspace.json files to find project paths
            // For now, we just use "Cursor" since workspace-to-session mapping
            // requires additional heuristics
            return "Cursor".to_string();
        }
    }

    "Cursor".to_string()
}

fn millis_to_datetime(ms: i64) -> DateTime<Utc> {
    let secs = ms / 1000;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    Utc.timestamp_opt(secs, nsecs).single().unwrap_or_else(Utc::now)
}
