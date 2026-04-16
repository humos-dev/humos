//! Async-friendly IPC client for the Tauri app.
//!
//! Uses raw JSON over a Unix socket to avoid the circular dependency that
//! would arise from importing humos-daemon types (daemon imports humos_lib
//! which is this crate). Protocol matches humos-daemon/src/ipc/protocol.rs.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);
const RIBBON_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_RIBBON_LIMIT: usize = 5;

fn socket_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir")
        .join(".humOS")
        .join("daemon.sock")
}

/// Low-level: send a JSON request line, read a JSON response line.
fn ipc_call(request: &Value, timeout: Duration) -> Result<Value, String> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path).map_err(|e| {
        format!(
            "daemon offline — {} — start with: humos-daemon serve",
            e
        )
    })?;
    stream.set_read_timeout(Some(timeout)).ok();
    stream.set_write_timeout(Some(timeout)).ok();

    let mut payload = serde_json::to_string(request).map_err(|e| e.to_string())?;
    payload.push('\n');
    stream
        .write_all(payload.as_bytes())
        .map_err(|e| format!("write error: {}", e))?;
    stream.flush().ok();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("read error: {}", e))?;
    if line.trim().is_empty() {
        return Err("daemon returned empty response".to_string());
    }
    serde_json::from_str(line.trim()).map_err(|e| format!("parse error: {}", e))
}

// ── Public types returned to Tauri commands ──────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct DaemonHealth {
    pub online: bool,
    pub index_sessions: u64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RibbonEntry {
    pub session_id: String,
    pub project: String,
    pub cwd: String,
    pub snippet: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RibbonResult {
    pub daemon_online: bool,
    pub is_stale: bool,
    pub entries: Vec<RibbonEntry>,
    pub total_count: u64,
}

// ── Internal response shapes ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct HealthResponse {
    ok: bool,
    index_sessions: u64,
    uptime_secs: u64,
}

#[derive(Deserialize)]
struct SearchMatch {
    id: String,
    project: String,
    cwd: String,
    snippet: String,
    modified_at: String,
}

#[derive(Deserialize)]
struct RelatedContextResponse {
    matches: Vec<SearchMatch>,
    total_count: u64,
    is_stale: bool,
    daemon_online: bool,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Poll daemon health (blocking). Call via spawn_blocking from async handlers.
pub fn poll_health() -> DaemonHealth {
    let req = json!({"type": "health"});
    match ipc_call(&req, HEALTH_TIMEOUT) {
        Ok(resp) => {
            if resp.get("type").and_then(|t| t.as_str()) == Some("health") {
                if let Ok(h) = serde_json::from_value::<HealthResponse>(resp) {
                    log::info!(
                        "[daemon_client] connected to {} — ok={}, sessions={}, uptime={}s",
                        socket_path().display(),
                        h.ok,
                        h.index_sessions,
                        h.uptime_secs
                    );
                    return DaemonHealth {
                        online: h.ok,
                        index_sessions: h.index_sessions,
                        uptime_secs: h.uptime_secs,
                    };
                }
            }
            log::warn!("[daemon_client] unexpected health response");
            DaemonHealth { online: false, index_sessions: 0, uptime_secs: 0 }
        }
        Err(e) => {
            log::warn!("[daemon_client] lost connection — retrying in 5s: {}", e);
            DaemonHealth { online: false, index_sessions: 0, uptime_secs: 0 }
        }
    }
}

/// Fetch related context for a cwd (blocking). Returns ribbon payload.
/// Empty cwd → returns offline/empty result without an IPC call.
pub fn fetch_related_context(cwd: &str) -> RibbonResult {
    if cwd.is_empty() {
        return RibbonResult {
            daemon_online: false,
            is_stale: false,
            entries: vec![],
            total_count: 0,
        };
    }

    let req = json!({
        "type": "related_context",
        "cwd": cwd,
        "limit": DEFAULT_RIBBON_LIMIT,
    });

    match ipc_call(&req, RIBBON_TIMEOUT) {
        Ok(resp) => {
            let resp_type = resp.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if resp_type == "related_context" {
                match serde_json::from_value::<RelatedContextResponse>(resp) {
                    Ok(r) => {
                        log::info!(
                            "[ribbon] RelatedContext for {}: {} results, is_stale={}",
                            cwd,
                            r.matches.len(),
                            r.is_stale
                        );
                        let entries = r
                            .matches
                            .into_iter()
                            .map(|m| RibbonEntry {
                                session_id: m.id,
                                project: m.project,
                                cwd: m.cwd,
                                snippet: truncate_snippet(&m.snippet, 80),
                                modified_at: m.modified_at,
                            })
                            .collect();
                        return RibbonResult {
                            daemon_online: r.daemon_online,
                            is_stale: r.is_stale,
                            entries,
                            total_count: r.total_count,
                        };
                    }
                    Err(e) => {
                        log::warn!("[ribbon] parse error for {}: {}", cwd, e);
                    }
                }
            } else if resp_type == "error" {
                let problem = resp
                    .get("problem")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown error");
                log::warn!("[ribbon] daemon error for {}: {}", cwd, problem);
                return RibbonResult {
                    daemon_online: true,
                    is_stale: false,
                    entries: vec![],
                    total_count: 0,
                };
            }
            RibbonResult {
                daemon_online: false,
                is_stale: false,
                entries: vec![],
                total_count: 0,
            }
        }
        Err(e) => {
            log::warn!("[ribbon] IPC failed for {}: {}", cwd, e);
            RibbonResult {
                daemon_online: false,
                is_stale: false,
                entries: vec![],
                total_count: 0,
            }
        }
    }
}

fn truncate_snippet(s: &str, max_chars: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated.trim_end())
    }
}
