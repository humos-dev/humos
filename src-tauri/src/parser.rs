use std::fs;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::SessionState;

/// Raw JSONL line shapes we care about.
#[derive(Deserialize, Debug)]
struct RawLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    subtype: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    message: Option<Value>,
    timestamp: Option<String>,
}

/// Parse a `.jsonl` session file into a `SessionState`.
/// Returns `None` if the file cannot be read or has no usable content.
pub fn parse_session_file(path: &Path) -> Option<SessionState> {
    let content = fs::read_to_string(path).ok()?;
    let filename = path.file_stem()?.to_string_lossy().to_string();

    let modified_at = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(system_time_to_rfc3339)
        .unwrap_or_default();

    let mut cwd = String::new();
    let mut started_at = String::new();
    let mut last_output = String::new();
    let mut tool_count: u32 = 0;
    let mut last_role: Option<String> = None;
    let mut session_id = filename.clone();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parsed: RawLine = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Init line — grab cwd and session id
        if parsed.kind.as_deref() == Some("system")
            && parsed.subtype.as_deref() == Some("init")
        {
            if let Some(c) = &parsed.cwd {
                cwd = c.clone();
            }
            if let Some(id) = &parsed.session_id {
                session_id = id.clone();
            }
            if started_at.is_empty() {
                started_at = parsed.timestamp.clone().unwrap_or_default();
            }
            continue;
        }

        // Message lines
        if let Some(msg) = &parsed.message {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("").to_string();
            last_role = Some(role.clone());

            if role == "assistant" {
                if let Some(content_arr) = msg.get("content").and_then(|c| c.as_array()) {
                    for item in content_arr {
                        let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match item_type {
                            "tool_use" => {
                                tool_count += 1;
                                let tool_name = item
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("unknown");
                                last_output = format!("Running: {}", tool_name);
                            }
                            "text" => {
                                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                    let trimmed = text.trim();
                                    if !trimmed.is_empty() {
                                        last_output = truncate(trimmed, 120).to_string();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Derive project from last segment of cwd
    let project = if cwd.is_empty() {
        filename.clone()
    } else {
        Path::new(&cwd)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| cwd.clone())
    };

    // Status heuristic: parse modified_at and compare with now
    let status = compute_status(&modified_at, last_role.as_deref());

    Some(SessionState {
        id: session_id,
        project,
        cwd,
        status,
        last_output,
        tool_count,
        started_at,
        modified_at,
    })
}

fn compute_status(modified_at: &str, last_role: Option<&str>) -> String {
    let age_secs = modified_at_age_secs(modified_at);
    if age_secs < 300 {
        match last_role {
            Some("assistant") => "running".to_string(),
            Some("user") => "waiting".to_string(),
            _ => "idle".to_string(),
        }
    } else {
        "idle".to_string()
    }
}

fn modified_at_age_secs(ts: &str) -> u64 {
    if ts.is_empty() {
        return u64::MAX;
    }
    let parsed = ts.parse::<DateTime<Utc>>().ok();
    match parsed {
        Some(dt) => {
            let now = Utc::now();
            (now - dt).num_seconds().max(0) as u64
        }
        None => u64::MAX,
    }
}

fn system_time_to_rfc3339(st: SystemTime) -> String {
    let dt: DateTime<Utc> = st.into();
    dt.to_rfc3339()
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Truncate at char boundary
        let mut idx = max;
        while !s.is_char_boundary(idx) {
            idx -= 1;
        }
        &s[..idx]
    }
}
