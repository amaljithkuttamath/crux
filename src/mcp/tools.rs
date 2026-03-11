use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SessionIdInput {
    #[schemars(description = "Session ID to analyze. If omitted, uses the most recent active session.")]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListSessionsInput {
    #[schemars(description = "Maximum number of sessions to return. Defaults to 10.")]
    pub limit: Option<usize>,
    #[schemars(description = "Filter by project name (partial match).")]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSessionsInput {
    #[schemars(description = "Keyword to search for in session topics and project names.")]
    pub query: String,
    #[schemars(description = "Maximum number of results. Defaults to 10.")]
    pub limit: Option<usize>,
}
