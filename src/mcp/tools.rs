use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SessionIdInput {
    #[schemars(description = "Session ID to analyze. If omitted, uses the most recent active session.")]
    pub session_id: Option<String>,
}
