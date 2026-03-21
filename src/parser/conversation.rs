use crate::parser::Source;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

/// Lightweight session metadata, kept in memory for the session list.
/// ~200 bytes per session. No conversation content stored.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionMeta {
    pub session_id: String,
    pub project: String,
    pub file_path: String,
    pub first_message: String,
    pub source: Source,
    pub message_count: usize,
    pub user_count: usize,
    pub assistant_count: usize,
    pub tools_used: Vec<String>,
    pub tool_counts: HashMap<String, usize>,
    pub agent_spawns: usize,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

impl SessionMeta {
    pub fn duration_minutes(&self) -> i64 {
        (self.end_time - self.start_time).num_minutes()
    }
}

/// Full conversation detail, loaded lazily when user drills in.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConversationMessage {
    pub timestamp: DateTime<Utc>,
    pub role: String,
    pub content: String,
    pub tool_names: Vec<String>,
}

#[derive(Deserialize)]
struct RawLine {
    #[serde(rename = "type")]
    record_type: Option<String>,
    timestamp: Option<String>,
    message: Option<serde_json::Value>,
}

/// Fast scan: extract only metadata. No message content stored.
pub fn parse_session_meta(path: &str) -> anyhow::Result<SessionMeta> {
    let content = std::fs::read_to_string(path)?;
    let project = super::extract_project_name(path);
    let session_id = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let mut first_message = String::new();
    let mut user_count = 0usize;
    let mut assistant_count = 0usize;
    let mut tool_counts: HashMap<String, usize> = HashMap::new();
    let mut start_time: Option<DateTime<Utc>> = None;
    let mut end_time: Option<DateTime<Utc>> = None;

    for line in content.lines() {
        let parsed: RawLine = match serde_json::from_str(line) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let record_type = match parsed.record_type.as_deref() {
            Some(t) => t,
            None => continue,
        };

        if let Some(ts_str) = &parsed.timestamp {
            if let Ok(t) = DateTime::parse_from_rfc3339(ts_str) {
                let t = t.with_timezone(&Utc);
                if start_time.is_none_or(|s| t < s) { start_time = Some(t); }
                if end_time.is_none_or(|e| t > e) { end_time = Some(t); }
            }
        }

        match record_type {
            "user" => {
                user_count += 1;
                if first_message.is_empty() {
                    let text = extract_user_text(&parsed.message);
                    if !text.is_empty() {
                        first_message = truncate_str(&text, 120);
                    }
                }
            }
            "assistant" => {
                assistant_count += 1;
                extract_tool_names(&parsed.message, &mut tool_counts);
            }
            _ => {}
        }
    }

    let mut tools_used: Vec<String> = tool_counts.keys().cloned().collect();
    tools_used.sort_by(|a, b| {
        tool_counts.get(b).unwrap_or(&0).cmp(tool_counts.get(a).unwrap_or(&0))
    });

    let agent_spawns = tool_counts.get("Agent").copied().unwrap_or(0);

    let now = Utc::now();
    Ok(SessionMeta {
        session_id,
        project,
        file_path: path.to_string(),
        first_message,
        source: Source::ClaudeCode,
        message_count: user_count + assistant_count,
        user_count,
        assistant_count,
        tools_used,
        tool_counts,
        agent_spawns,
        start_time: start_time.unwrap_or(now),
        end_time: end_time.unwrap_or(now),
    })
}

/// Full parse: load all user messages for detail view. Called lazily.
pub fn parse_conversation(path: &str) -> anyhow::Result<Vec<ConversationMessage>> {
    let content = std::fs::read_to_string(path)?;
    let mut messages = Vec::new();

    for line in content.lines() {
        let parsed: RawLine = match serde_json::from_str(line) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let record_type = match parsed.record_type.as_deref() {
            Some(t) => t,
            None => continue,
        };

        let ts = match &parsed.timestamp {
            Some(s) => match DateTime::parse_from_rfc3339(s) {
                Ok(t) => t.with_timezone(&Utc),
                Err(_) => continue,
            },
            None => continue,
        };

        match record_type {
            "user" => {
                let text = extract_user_text(&parsed.message);
                if !text.is_empty() {
                    messages.push(ConversationMessage {
                        timestamp: ts,
                        role: "user".to_string(),
                        content: truncate_str(&text, 300),
                        tool_names: Vec::new(),
                    });
                }
            }
            "assistant" => {
                let (text, tools) = extract_assistant_content(&parsed.message);
                if !text.is_empty() || !tools.is_empty() {
                    messages.push(ConversationMessage {
                        timestamp: ts,
                        role: "assistant".to_string(),
                        content: truncate_str(&text, 300),
                        tool_names: tools,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(messages)
}

fn extract_user_text(message: &Option<serde_json::Value>) -> String {
    let msg = match message {
        Some(v) => v,
        None => return String::new(),
    };
    if let Some(content) = msg.get("content") {
        if let Some(s) = content.as_str() {
            return s.trim().to_string();
        }
        if let Some(arr) = content.as_array() {
            for block in arr {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        return text.trim().to_string();
                    }
                }
            }
        }
    }
    String::new()
}

fn extract_tool_names(message: &Option<serde_json::Value>, counts: &mut HashMap<String, usize>) {
    let msg = match message {
        Some(v) => v,
        None => return,
    };
    if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                    *counts.entry(name.to_string()).or_default() += 1;
                }
            }
        }
    }
}

fn extract_assistant_content(message: &Option<serde_json::Value>) -> (String, Vec<String>) {
    let msg = match message {
        Some(v) => v,
        None => return (String::new(), Vec::new()),
    };
    let mut text = String::new();
    let mut tools = Vec::new();

    if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
        for block in content {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if text.is_empty() {
                        if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                            text = t.trim().to_string();
                        }
                    }
                }
                Some("tool_use") => {
                    if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                        tools.push(name.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    (text, tools)
}

fn truncate_str(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() > max {
        format!("{}...", &first_line[..max.saturating_sub(3)])
    } else {
        first_line.to_string()
    }
}
