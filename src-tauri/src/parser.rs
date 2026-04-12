use std::fs;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::SessionState;

/// Raw JSONL line — all fields are optional because not every line has all of them.
#[derive(Deserialize, Debug)]
struct RawLine {
    /// Top-level event type: "user", "assistant", "progress", "file-history-snapshot", etc.
    #[serde(rename = "type")]
    kind: Option<String>,

    /// Present on most lines — the working directory for the session.
    cwd: Option<String>,

    /// Present on most lines — the session UUID.
    #[serde(rename = "sessionId")]
    session_id: Option<String>,

    /// The message payload (for "user" and "assistant" type lines).
    message: Option<Value>,

    /// Timestamp of this event.
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
    let mut recent_tools: Vec<String> = Vec::new();
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

        // Grab cwd and session_id from whichever line has them first.
        if cwd.is_empty() {
            if let Some(c) = &parsed.cwd {
                if !c.is_empty() {
                    cwd = c.clone();
                }
            }
        }
        if session_id == filename {
            if let Some(id) = &parsed.session_id {
                if !id.is_empty() {
                    session_id = id.clone();
                }
            }
        }
        if started_at.is_empty() {
            if let Some(ts) = &parsed.timestamp {
                started_at = ts.clone();
            }
        }

        // Only process user/assistant event lines for status tracking.
        let kind = parsed.kind.as_deref().unwrap_or("");
        if kind != "user" && kind != "assistant" {
            continue;
        }

        last_role = Some(kind.to_string());

        if kind == "assistant" {
            if let Some(msg) = &parsed.message {
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
                                recent_tools.push(tool_name.to_string());
                                if recent_tools.len() > 5 {
                                    recent_tools.remove(0);
                                }
                            }
                            "text" => {
                                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                    let trimmed = text.trim();
                                    if !trimmed.is_empty() {
                                        last_output = truncate(trimmed, 500).to_string();
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

    // Require at least a cwd or session_id to consider this a valid session.
    if cwd.is_empty() && session_id == filename {
        return None;
    }

    // Derive project name from last path segment of cwd.
    let project = if cwd.is_empty() {
        filename.clone()
    } else {
        Path::new(&cwd)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| cwd.clone())
    };

    // Compute file age for the status gate.
    let modified_age_secs = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|mtime| mtime.elapsed().ok())
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX);

    let status = compute_status(last_role.as_deref(), modified_age_secs);

    // Resolve the tty for this session by finding a Claude process whose
    // Terminal tab we can match. Empty string if unresolvable.
    let tty = resolve_tty_for_session(&cwd);

    Some(SessionState {
        id: session_id,
        project,
        cwd,
        status,
        last_output,
        tool_count,
        recent_tools,
        tty,
        started_at,
        modified_at,
    })
}

/// Resolve which tty device a Claude session is running on.
/// Returns the tty path (e.g. "/dev/ttys001") or empty string if unknown.
///
/// Uses osascript to check Terminal window names against the session's cwd.
/// Falls back to the first available Claude tty when there's only one session.
fn resolve_tty_for_session(cwd: &str) -> String {
    if cwd.is_empty() {
        return String::new();
    }
    let last_segment = cwd.split('/').filter(|s| !s.is_empty()).last().unwrap_or("");
    if last_segment.is_empty() {
        return String::new();
    }

    // Ask Terminal for all windows with their ttys and names
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(r#"
tell application "Terminal"
    set r to ""
    repeat with w in windows
        try
            set wName to name of w
            repeat with t in tabs of w
                set tTty to tty of t
                set r to r & tTty & "|||" & wName & linefeed
            end repeat
        end try
    end repeat
    return r
end tell"#)
        .output();

    let stdout = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return String::new(),
    };

    // Try to match by CWD substring in window name
    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(2, "|||").collect();
        if parts.len() != 2 { continue; }
        let tty = parts[0].trim();
        let name = parts[1].trim();
        if name.to_lowercase().contains(&last_segment.to_lowercase()) {
            return tty.to_string();
        }
    }

    // If only one Terminal tab has claude, use it (unambiguous)
    let claude_ttys: Vec<&str> = stdout.lines()
        .filter_map(|l| {
            let parts: Vec<&str> = l.splitn(2, "|||").collect();
            if parts.len() == 2 && parts[1].contains("claude") {
                Some(parts[0].trim())
            } else {
                None
            }
        })
        .collect();

    if claude_ttys.len() == 1 {
        return claude_ttys[0].to_string();
    }

    String::new()
}

/// Derive session status from the last message role and file age.
///
/// Both "running" and "waiting" require the file to have been modified
/// recently (within 5 minutes). A session whose JSONL hasn't been touched
/// in hours is dead regardless of what the last role was — the Terminal
/// tab was likely closed without Claude writing a final entry.
fn compute_status(last_role: Option<&str>, modified_age_secs: u64) -> String {
    match last_role {
        Some("assistant") if modified_age_secs < 300 => "running".to_string(),
        Some("user") if modified_age_secs < 300 => "waiting".to_string(),
        _ => "idle".to_string(),
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
        let mut idx = max;
        while !s.is_char_boundary(idx) {
            idx -= 1;
        }
        &s[..idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn recent_ts() -> String {
        Utc::now().to_rfc3339()
    }

    fn old_ts() -> String {
        (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339()
    }

    fn write_jsonl(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        f.flush().unwrap();
        f
    }

    // Helper: parse file and set a fake modified_at via compute_status directly.
    // Because file mtime is always "now" for a just-created tempfile, we test
    // compute_status independently for old-timestamp cases.

    #[test]
    fn test_user_turn_recent_is_waiting() {
        let ts = recent_ts();
        let line = format!(
            r#"{{"type":"user","cwd":"/tmp/proj","sessionId":"abc-123","timestamp":"{}","message":{{"role":"user","content":[{{"type":"text","text":"hello"}}]}}}}"#,
            ts
        );
        let f = write_jsonl(&[&line]);
        let result = parse_session_file(f.path()).unwrap();
        assert_eq!(result.status, "waiting");
        assert_eq!(result.cwd, "/tmp/proj");
        assert_eq!(result.id, "abc-123");
    }

    #[test]
    fn test_assistant_turn_recent_is_running() {
        let ts = recent_ts();
        let line = format!(
            r#"{{"type":"assistant","cwd":"/tmp/proj","sessionId":"def-456","timestamp":"{}","message":{{"role":"assistant","content":[{{"type":"text","text":"I am helping"}}]}}}}"#,
            ts
        );
        let f = write_jsonl(&[&line]);
        let result = parse_session_file(f.path()).unwrap();
        assert_eq!(result.status, "running");
    }

    #[test]
    fn test_status_uses_mtime_gate() {
        // Recent assistant turn → running
        assert_eq!(compute_status(Some("assistant"), 10), "running");
        // Old assistant turn (>5 min) → idle (session finished)
        assert_eq!(compute_status(Some("assistant"), 600), "idle");
        // Recent user turn → waiting
        assert_eq!(compute_status(Some("user"), 0), "waiting");
        assert_eq!(compute_status(Some("user"), 60), "waiting");
        // Old user turn (>5 min) → idle (terminal tab likely closed)
        assert_eq!(compute_status(Some("user"), 600), "idle");
        assert_eq!(compute_status(Some("user"), 9999), "idle");
        // No role → idle
        assert_eq!(compute_status(None, 0), "idle");
    }

    #[test]
    fn test_empty_file_returns_none() {
        let f = write_jsonl(&[]);
        let result = parse_session_file(f.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_all_malformed_lines_returns_none() {
        let f = write_jsonl(&["not json at all", "{broken", "also bad"]);
        let result = parse_session_file(f.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_no_cwd_session_id_from_filename() {
        // No cwd in the line, but sessionId present — should use sessionId
        let ts = recent_ts();
        // The filename stem will be the temp file name; sessionId overrides it
        let line = format!(
            r#"{{"type":"user","sessionId":"override-id","timestamp":"{}","message":{{"role":"user","content":[]}}}}"#,
            ts
        );
        // Provide a second line with cwd so None isn't returned
        let cwd_line = format!(
            r#"{{"type":"user","cwd":"/tmp/somewhere","sessionId":"override-id","timestamp":"{}","message":{{"role":"user","content":[]}}}}"#,
            ts
        );
        let f = write_jsonl(&[&line, &cwd_line]);
        let result = parse_session_file(f.path()).unwrap();
        assert_eq!(result.id, "override-id");
    }

    #[test]
    fn test_tool_use_increments_tool_count() {
        let ts = recent_ts();
        let line1 = format!(
            r#"{{"type":"assistant","cwd":"/tmp/proj","sessionId":"tool-test","timestamp":"{}","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"bash"}},{{"type":"tool_use","name":"read_file"}}]}}}}"#,
            ts
        );
        let f = write_jsonl(&[&line1]);
        let result = parse_session_file(f.path()).unwrap();
        assert_eq!(result.tool_count, 2);
    }
}
