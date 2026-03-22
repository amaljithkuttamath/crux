use std::collections::HashMap;
use std::path::Path;

/// Check which Claude Code sessions are currently live by reading
/// ~/.claude/sessions/*.json and checking if the PIDs are running.
pub fn check_liveness(sessions_dir: &Path) -> HashMap<String, bool> {
    let mut map = HashMap::new();

    let entries = match std::fs::read_dir(sessions_dir) {
        Ok(e) => e,
        Err(_) => return map,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let session_id = match json.get("sessionId").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        let pid = match json.get("pid").and_then(|v| v.as_i64()) {
            Some(p) => p as i32,
            None => continue,
        };

        let is_alive = unsafe { libc::kill(pid, 0) } == 0;
        map.insert(session_id, is_alive);
    }

    map
}
