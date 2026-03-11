pub mod watcher;
pub mod conversation;

use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

#[derive(Deserialize)]
struct JsonlLine {
    timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    usage: Option<Usage>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

pub fn parse_line(line: &str) -> Option<UsageRecord> {
    let parsed: JsonlLine = serde_json::from_str(line).ok()?;
    let message = parsed.message?;
    let usage = message.usage?;

    let input = usage.input_tokens.unwrap_or(0);
    let output = usage.output_tokens.unwrap_or(0);
    let cache_create = usage.cache_creation_input_tokens.unwrap_or(0);
    let cache_read = usage.cache_read_input_tokens.unwrap_or(0);

    if input == 0 && output == 0 && cache_create == 0 && cache_read == 0 {
        return None;
    }

    let timestamp = parsed
        .timestamp
        .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
        .map(|t| t.with_timezone(&Utc))?;

    Some(UsageRecord {
        timestamp,
        session_id: parsed.session_id.unwrap_or_default(),
        project: String::new(),
        model: message.model.unwrap_or_default(),
        input_tokens: input,
        output_tokens: output,
        cache_creation_tokens: cache_create,
        cache_read_tokens: cache_read,
    })
}

pub fn parse_file(path: &str) -> anyhow::Result<Vec<UsageRecord>> {
    let content = std::fs::read_to_string(path)?;
    let project = extract_project_name(path);
    let records: Vec<UsageRecord> = content
        .lines()
        .filter_map(|line| {
            let mut record = parse_line(line)?;
            record.project = project.clone();
            Some(record)
        })
        .collect();
    Ok(records)
}

fn extract_project_name(path: &str) -> String {
    std::path::Path::new(path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|name| {
            if let Some(idx) = name.find("-Developer-") {
                name[idx + 11..].to_string()
            } else if name.contains("-Developer") {
                "Developer".to_string()
            } else {
                name.to_string()
            }
        })
        .unwrap_or_default()
}
