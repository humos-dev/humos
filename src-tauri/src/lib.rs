mod applescript;
mod daemon_client;
mod parser;
pub mod pipe;
pub mod providers;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Global counter for unique rule IDs. Monotonically increasing, process-lifetime.
static RULE_COUNTER: AtomicU64 = AtomicU64::new(1);

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use providers::claude::ClaudeProvider;
use providers::opencode::OpenCodeProvider;
use providers::ProviderRegistry;

/// The shared session state, keyed by session id.
type SessionMap = Arc<Mutex<HashMap<String, SessionState>>>;

/// Per-session result from a signal broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignalResult {
    session_id: String,
    project: String,
    success: bool,
    error: Option<String>,
}

/// Emitted after signal_sessions runs — carries per-session delivery status.
#[derive(Debug, Clone, Serialize)]
struct SignalFiredEvent {
    message: String,
    success_ids: Vec<String>,
    fail_ids: Vec<String>,
    success_count: usize,
    fail_count: usize,
}

/// Maximum age of session files scanned on startup and during the background
/// poll. Files older than this are skipped; they remain searchable via the
/// daemon index but won't appear as live dashboard cards.
const MAX_SESSION_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days

/// Background poll interval. Sessions are re-read from disk every 5s.
/// This replaces the file watcher + periodic rescan from before Phase C.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum message length accepted by `signal_sessions`, in characters.
/// Mirrors the UI limit — enforced server-side so any bypassing caller can't
/// slip through with a multi-megabyte payload.
const SIGNAL_MAX_CHARS: usize = 512;

/// Tauri command: broadcast a message to every non-idle session.
///
/// Iterates all sessions where `status != "idle"` (running + waiting), calls
/// `inject_message` for each, then emits a `signal-fired` event so the UI can
/// animate and log the result.  Partial failure is handled gracefully — results
/// for each session are returned regardless of whether injection succeeded.
///
/// Message content is validated server-side: trimmed, non-empty, capped at
/// SIGNAL_MAX_CHARS characters (not bytes), and stripped of control characters
/// (newlines/tabs would fragment the Claude CLI input mid-prompt).
#[tauri::command]
async fn signal_sessions(
    message: String,
    state: State<'_, SessionMap>,
    app: AppHandle,
) -> Result<Vec<SignalResult>, String> {
    // Validate and sanitize the message.
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err("Message is empty.".to_string());
    }
    if trimmed.chars().count() > SIGNAL_MAX_CHARS {
        return Err(format!("Message exceeds {} character limit.", SIGNAL_MAX_CHARS));
    }
    // Replace newlines/tabs/other control chars with spaces so the message lands
    // in the Claude CLI prompt as a single line rather than submitting mid-draft.
    let sanitized: String = trimmed
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    // Snapshot non-idle sessions while holding the lock, then drop it immediately.
    let targets: Vec<(String, String, String)> = {
        let sessions = state.lock().unwrap_or_else(|e| e.into_inner());
        sessions
            .values()
            .filter(|s| s.status != "idle")
            .map(|s| (s.id.clone(), s.project.clone(), s.cwd.clone()))
            .collect()
    };

    if targets.is_empty() {
        return Err("No active sessions.".to_string());
    }

    // Signal broadcasts to every registered provider's terminal tabs at once.
    // Each provider matches its own process name (claude, opencode, etc.) so
    // a single signal fans across every supported agent CLI.
    let msg_for_blocking = sanitized.clone();
    let targets_for_blocking = targets.clone();
    let results: Vec<SignalResult> = tokio::task::spawn_blocking(move || {
        let broadcast_result = build_provider_registry().broadcast(&msg_for_blocking);
        match broadcast_result {
            Ok(count) => {
                log::info!("signal: broadcast to {} terminal tabs", count);
                // Mark all targeted sessions as successful since the broadcast
                // injects into every tab with claude, not per-session.
                targets_for_blocking.iter().map(|(id, project, _cwd)| {
                    SignalResult {
                        session_id: id.clone(),
                        project: project.clone(),
                        success: true,
                        error: None,
                    }
                }).collect()
            }
            Err(e) => {
                log::error!("signal: broadcast failed: {}", e);
                targets_for_blocking.iter().map(|(id, project, _cwd)| {
                    SignalResult {
                        session_id: id.clone(),
                        project: project.clone(),
                        success: false,
                        error: Some(e.clone()),
                    }
                }).collect()
            }
        }
    })
    .await
    .map_err(|e| format!("signal broadcast task panicked: {}", e))?;

    let success_ids: Vec<String> = results
        .iter()
        .filter(|r| r.success)
        .map(|r| r.session_id.clone())
        .collect();
    let fail_ids: Vec<String> = results
        .iter()
        .filter(|r| !r.success)
        .map(|r| r.session_id.clone())
        .collect();

    let evt = SignalFiredEvent {
        message: sanitized.clone(),
        success_count: success_ids.len(),
        fail_count: fail_ids.len(),
        success_ids,
        fail_ids,
    };

    if let Err(e) = app.emit("signal-fired", &evt) {
        log::error!("emit signal-fired error: {}", e);
    }

    // Preview: char-based truncation (not byte slice) — avoids panic on
    // multi-byte UTF-8 boundaries (emoji, non-ASCII).
    let preview: String = sanitized.chars().take(60).collect();
    log::info!(
        "signal: broadcast to {} sessions ({} ok, {} failed): {}",
        targets.len(),
        evt.success_count,
        evt.fail_count,
        preview
    );

    Ok(results)
}

/// Emitted when a pipe rule fires and a message is about to be injected.
#[derive(serde::Serialize, Clone)]
struct PipeFired {
    rule_id: String,
    from_session_id: String,
    to_session_id: String,
    message: String,
    success: bool,           // add this
    error: Option<String>,   // add this
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub id: String,
    pub project: String,
    pub cwd: String,
    pub status: String,
    pub last_output: String,
    pub tool_count: u32,
    pub recent_tools: Vec<String>,
    pub tty: String,
    pub started_at: String,
    pub modified_at: String,
    pub provider: String,
}

/// Tauri command: return all known sessions as a sorted Vec.
#[tauri::command]
fn get_sessions(state: State<'_, SessionMap>) -> Vec<SessionState> {
    let map = state.lock().unwrap_or_else(|e| e.into_inner());
    let mut sessions: Vec<SessionState> = map.values().cloned().collect();
    // Sort: running first, waiting second, idle last; then by modified_at desc
    sessions.sort_by(|a, b| {
        status_order(&a.status)
            .cmp(&status_order(&b.status))
            .then(b.modified_at.cmp(&a.modified_at))
    });
    sessions
}

fn status_order(s: &str) -> u8 {
    match s {
        "running" => 0,
        "waiting" => 1,
        _ => 2,
    }
}

///// Tauri command: focus the Terminal window for a session and bring it to front.
#[tauri::command]
fn focus_session(
    session_id: String,
    cwd: Option<String>,
    state: State<'_, SessionMap>,
) -> Result<(), String> {
    let resolved_cwd = {
        let map = state.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&session_id).map(|s| s.cwd.clone())
    };

    let effective_cwd = resolved_cwd
        .or_else(|| cwd.filter(|c| !c.is_empty()))
        .ok_or_else(|| format!("Session {} not found or has no cwd", session_id))?;

    applescript::focus_terminal(&effective_cwd)
}

/// Tauri command: inject a message into the terminal for a session.
///
/// Session IDs (JSONL filenames) change every time Claude CLI restarts, so
/// we accept an optional `cwd` fallback from the frontend. Resolution order:
///   1. look up session_id in the map and use its current cwd
///   2. if that fails, use the explicit cwd param the caller passed
///   3. if both are empty, return an error
///
/// This mirrors the pipe() CWD fallback fix from v0.3.2 — the Send button
/// and every other single-session inject path now survives session restarts.
#[tauri::command]
fn inject_message(
    session_id: String,
    message: String,
    cwd: Option<String>,
    state: State<'_, SessionMap>,
) -> Result<(), String> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err("Message is empty.".to_string());
    }
    if trimmed.chars().count() > SIGNAL_MAX_CHARS {
        return Err(format!("Message exceeds {} character limit.", SIGNAL_MAX_CHARS));
    }
    let sanitized: String = trimmed
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    let resolved_cwd = {
        let map = state.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&session_id).map(|s| s.cwd.clone())
    };

    let effective_cwd = resolved_cwd
        .or_else(|| cwd.filter(|c| !c.is_empty()))
        .ok_or_else(|| format!(
            "Session {} not found. If Claude CLI was restarted, the session ID changed; humOS will recover on the next poll.",
            session_id
        ))?;

    applescript::inject_message(&effective_cwd, &sanitized)
}

/// Tauri command: add a pipe rule. Returns the rule id on success.
///
/// `from_cwd` and `to_cwd` are captured at rule-creation time so that pipe
/// evaluation can fall back to CWD matching when session IDs change (which
/// happens every time Claude CLI restarts — IDs are JSONL filenames).
#[tauri::command]
fn add_pipe_rule(
    from_session_id: String,
    to_session_id: String,
    from_cwd: Option<String>,
    to_cwd: Option<String>,
    trigger: String,
    file_pattern: Option<String>,
    pipe_manager: State<'_, Arc<Mutex<pipe::PipeManager>>>,
    sessions: State<'_, SessionMap>,
) -> Result<String, String> {
    let trigger = match trigger.as_str() {
        "on_idle" => pipe::PipeTrigger::OnIdle,
        "on_file_write" => {
            let pattern = file_pattern
                .filter(|p| !p.is_empty())
                .ok_or_else(|| "file_pattern is required for on_file_write trigger".to_string())?;
            pipe::PipeTrigger::OnFileWrite(pattern)
        }
        other => return Err(format!("unknown trigger type: {}", other)),
    };

    // Resolve CWDs: caller may supply them directly, or we look them up from the session map.
    let (resolved_from_cwd, resolved_to_cwd) = {
        let map = sessions.lock().unwrap_or_else(|e| e.into_inner());
        let fcwd = from_cwd.unwrap_or_default();
        let tcwd = to_cwd.unwrap_or_default();
        let from = if !fcwd.is_empty() {
            fcwd
        } else {
            map.get(&from_session_id).map(|s| s.cwd.clone()).unwrap_or_default()
        };
        let to = if !tcwd.is_empty() {
            tcwd
        } else {
            map.get(&to_session_id).map(|s| s.cwd.clone()).unwrap_or_default()
        };
        (from, to)
    };

    let id = format!("rule-{}", RULE_COUNTER.fetch_add(1, Ordering::SeqCst));

    let rule = pipe::PipeRule {
        id: id.clone(),
        from_session_id,
        to_session_id,
        from_cwd: resolved_from_cwd,
        to_cwd: resolved_to_cwd,
        trigger,
    };

    let mut mgr = pipe_manager.lock().map_err(|e| format!("lock error: {}", e))?;
    mgr.add_rule(rule);
    save_pipe_rules(&mgr);
    log::info!("pipe: added rule {}", id);
    Ok(id)
}

/// Tauri command: remove a pipe rule by id.
#[tauri::command]
fn remove_pipe_rule(
    rule_id: String,
    pipe_manager: State<'_, Arc<Mutex<pipe::PipeManager>>>,
) -> Result<(), String> {
    let mut mgr = pipe_manager.lock().map_err(|e| format!("lock error: {}", e))?;
    mgr.remove_rule(&rule_id);
    save_pipe_rules(&mgr);
    log::info!("pipe: removed rule {}", rule_id);
    Ok(())
}

/// Tauri command: list all active pipe rules.
#[tauri::command]
fn list_pipe_rules(
    pipe_manager: State<'_, Arc<Mutex<pipe::PipeManager>>>,
) -> Result<Vec<pipe::PipeRule>, String> {
    let mgr = pipe_manager.lock().map_err(|e| format!("lock error: {}", e))?;
    Ok(mgr.rules.clone())
}

/// Tauri command: summarize a session in plain English using Claude Haiku.
#[tauri::command]
async fn summarize_session(session_id: String) -> Result<String, String> {
    let projects_dir = claude_projects_dir()
        .ok_or_else(|| "Cannot determine ~/.claude/projects directory".to_string())?;

    let jsonl_path = find_jsonl(&projects_dir, &session_id)
        .ok_or_else(|| format!("Session file not found for id: {}", session_id))?;

    let content = fs::read_to_string(&jsonl_path)
        .map_err(|e| format!("Failed to read session file: {}", e))?;

    // Extract the last 50 meaningful lines (user/assistant messages only)
    let readable: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let obj: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
            let kind = obj.get("type")?.as_str()?;
            if kind != "user" && kind != "assistant" {
                return None;
            }
            let msg = obj.get("message")?;
            let role = msg.get("role")?.as_str()?;
            let content_arr = msg.get("content")?.as_array()?;
            let mut parts: Vec<String> = Vec::new();
            for item in content_arr {
                let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match item_type {
                    "text" => {
                        if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                            let trimmed = t.trim();
                            if !trimmed.is_empty() {
                                parts.push(trimmed.chars().take(300).collect());
                            }
                        }
                    }
                    "tool_use" => {
                        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                        parts.push(format!("[tool: {}]", name));
                    }
                    "tool_result" => {
                        parts.push("[tool result]".to_string());
                    }
                    _ => {}
                }
            }
            if parts.is_empty() {
                return None;
            }
            Some(format!("{}: {}", role.to_uppercase(), parts.join(" ")))
        })
        .collect();

    let last_50: Vec<&str> = readable.iter().rev().take(50).map(|s| s.as_str()).collect::<Vec<_>>().into_iter().rev().collect();
    let context = last_50.join("\n");

    if context.is_empty() {
        return Ok("No activity to summarize yet.".to_string());
    }

    let prompt = format!(
        "Here is the recent activity from a Claude CLI session. Summarize in 2 plain English sentences what this session is working on right now. Be specific and concrete — name the actual task, files, or goal. No preamble, no bullet points, just 2 sentences.\n\n---\n{}\n---",
        context
    );

    // Use the local `claude` CLI (already authenticated) to generate the summary.
    let claude_bin = which_claude();
    let output = tokio::process::Command::new(&claude_bin)
        .args(["-p", &prompt, "--no-session-persistence"])
        .output()
        .await
        .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("claude CLI error: {}", stderr.trim()));
    }

    let summary = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if summary.is_empty() { "No summary returned.".to_string() } else { summary })
}

/// Walk the projects directory and find a .jsonl whose stem matches session_id.
fn find_jsonl(base: &PathBuf, session_id: &str) -> Option<PathBuf> {
    for entry in walkdir_recursive(base) {
        let path = entry;
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if path.file_stem().and_then(|s| s.to_str()) == Some(session_id) {
                return Some(path);
            }
        }
    }
    None
}

/// Find the `claude` CLI binary — checks common install locations.
fn which_claude() -> String {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local/bin/claude").to_string_lossy().to_string());
        candidates.push(home.join(".cargo/bin/claude").to_string_lossy().to_string());
    }
    candidates.push("/usr/local/bin/claude".to_string());
    candidates.push("/opt/homebrew/bin/claude".to_string());
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return c.clone();
        }
    }
    "claude".to_string()
}

/// Returns the ~/.claude/projects path.
fn claude_projects_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".claude").join("projects"))
}

/// Simple recursive directory walker returning file paths.
fn walkdir_recursive(dir: &PathBuf) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir_recursive(&path));
            } else {
                result.push(path);
            }
        }
    }
    result
}

/// Build the provider registry with every supported agent CLI.
fn build_provider_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(ClaudeProvider::new()));
    registry.register(Box::new(OpenCodeProvider::new()));
    registry
}

/// Merge scanned sessions into the map keyed by session id, keeping the most
/// recently modified entry per id.
///
/// Why: when a Claude session is resumed (`claude --resume <id>`) a new JSONL
/// file is created with the SAME session id stamped inside. Without this rule,
/// last-write-wins HashMap insertion can mask the active resume with an older
/// file from the same session, leaving the dashboard stuck showing idle while
/// the user is actively chatting.
///
/// `modified_at` is RFC3339, emitted by `chrono::DateTime::to_rfc3339()` from
/// both parser.rs (file mtime) and opencode.rs (sqlite `time_updated`). Same
/// library, same format, so lexicographic comparison is safe today. If a
/// future provider emits a different RFC3339 variant (e.g. `Z` vs `+00:00`,
/// or a different timezone offset), switch to parsing into `DateTime<Utc>`
/// before comparing.
fn merge_sessions_by_newest(
    map: &mut HashMap<String, SessionState>,
    scanned: Vec<SessionState>,
) {
    for session in scanned {
        let keep_existing = match map.get(&session.id) {
            Some(existing) => existing.modified_at >= session.modified_at,
            None => false,
        };
        if !keep_existing {
            map.insert(session.id.clone(), session);
        }
    }
}

/// Scan every registered provider and reload the session map.
fn scan_sessions_into(sessions: &SessionMap) {
    let registry = build_provider_registry();
    let scanned = registry.scan_all(MAX_SESSION_AGE);
    let mut map = sessions.lock().unwrap_or_else(|e| e.into_inner());
    map.clear();
    merge_sessions_by_newest(&mut map, scanned);
    log::info!("session poll: loaded {} unique sessions across providers", map.len());
}

/// Tauri command: return current daemon health status.
#[tauri::command]
async fn check_daemon_health() -> Result<daemon_client::DaemonHealth, String> {
    tokio::task::spawn_blocking(daemon_client::poll_health)
        .await
        .map_err(|e| format!("health poll task panicked: {}", e))
}

/// Tauri command: fetch Project Brain ribbon for a focused session card.
/// Called with 200ms debounce from the frontend.
#[tauri::command]
async fn get_related_context(cwd: String) -> Result<daemon_client::RibbonResult, String> {
    tokio::task::spawn_blocking(move || daemon_client::fetch_related_context(&cwd))
        .await
        .map_err(|e| format!("ribbon task panicked: {}", e))
}

/// Path for persisted pipe rules: ~/.humOS/pipe-rules.json
fn pipe_rules_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join(".humOS");
    fs::create_dir_all(&dir).ok()?;
    Some(dir.join("pipe-rules.json"))
}

/// Persist current rules to disk. Errors are logged but not fatal.
fn save_pipe_rules(mgr: &pipe::PipeManager) {
    let Some(path) = pipe_rules_path() else { return };
    match serde_json::to_string_pretty(&mgr.rules) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                log::error!("pipe: failed to save rules to {:?}: {}", path, e);
            }
        }
        Err(e) => log::error!("pipe: failed to serialize rules: {}", e),
    }
}

/// Load persisted rules from disk and install them into the manager.
fn load_pipe_rules(mgr: &mut pipe::PipeManager) {
    let Some(path) = pipe_rules_path() else { return };
    let data = match fs::read_to_string(&path) {
        Ok(d) => d,
        Err(e) => {
            log::warn!("pipe: could not read rules from {:?}: {}", path, e);
            return;
        }
    };
    let rules: Vec<pipe::PipeRule> = match serde_json::from_str(&data) {
        Ok(r) => r,
        Err(e) => {
            log::error!("pipe: failed to deserialize rules: {}", e);
            return;
        }
    };
    let count = rules.len();
    for rule in rules {
        if let Some(n) = rule.id.strip_prefix("rule-").and_then(|s| s.parse::<u64>().ok()) {
            let current = RULE_COUNTER.load(Ordering::SeqCst);
            if n >= current {
                RULE_COUNTER.store(n + 1, Ordering::SeqCst);
            }
        }
        mgr.add_rule(rule);
    }
    log::info!("pipe: loaded {} persisted rules from {:?}", count, path);
}

/// Dispatch a pipe action. Always uses cwd-based injection so pipe fires
/// survive stale tty references (tab closed, rearranged, or claude
/// restarted). Pipe rules are Claude-scoped today; when other providers
/// gain pipe support we'll route through the registry here.
///
/// Tty-based precision injection is reserved for user-initiated actions
/// (Send button, manual inject) where the tty comes straight from the
/// currently-displayed session card and is known-fresh. Pipe dispatch is
/// ambient and a wrong injection is worse than a no-op, so we keep the
/// fuzzy cwd match that tolerates terminal churn.
fn dispatch_pipe_action(
    _sessions: &SessionMap,
    action: &pipe::PipeAction,
) -> Result<(), String> {
    applescript::inject_message(&action.target_cwd, &action.message)
}

/// Background thread: re-scan sessions every POLL_INTERVAL and evaluate pipe rules.
/// Replaces the file watcher + periodic rescan from before Phase C.
fn start_session_poll(
    app: AppHandle,
    sessions: SessionMap,
    pipe_manager: Arc<Mutex<pipe::PipeManager>>,
) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(POLL_INTERVAL);
            scan_sessions_into(&sessions);

            let actions = pipe::evaluate_pipes(&pipe_manager, &sessions);
            for action in actions {
                let inject_result = dispatch_pipe_action(&sessions, &action);
                let (success, error_msg) = match &inject_result {
                    Ok(()) => (true, None),
                    Err(e) => {
                        log::error!("pipe: inject failed for {}: {}", action.target_cwd, e);
                        (false, Some(e.clone()))
                    }
                };
                let fired_evt = PipeFired {
                    rule_id: action.rule_id.clone(),
                    from_session_id: action.from_session_id.clone(),
                    to_session_id: action.to_session_id.clone(),
                    message: action.message.clone(),
                    success,
                    error: error_msg,
                };
                let _ = app.emit("pipe-fired", &fired_evt);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let sessions: SessionMap = Arc::new(Mutex::new(HashMap::new()));
    let pipe_manager: Arc<Mutex<pipe::PipeManager>> = Arc::new(Mutex::new(pipe::PipeManager::new()));

    // Load persisted pipe rules before starting the poll.
    {
        let mut mgr = pipe_manager.lock().unwrap_or_else(|e| e.into_inner());
        load_pipe_rules(&mut mgr);
    }

    // Initial session scan.
    scan_sessions_into(&sessions);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(sessions.clone())
        .manage(pipe_manager.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            start_session_poll(
                handle,
                Arc::clone(&sessions),
                Arc::clone(&pipe_manager),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sessions,
            focus_session,
            inject_message,
            signal_sessions,
            summarize_session,
            add_pipe_rule,
            remove_pipe_rule,
            list_pipe_rules,
            check_daemon_health,
            get_related_context,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use pipe::{PipeManager, PipeRule, PipeTrigger};

    fn make_manager() -> PipeManager {
        PipeManager::new()
    }

    fn fixture_session(id: &str, cwd: &str, modified_at: &str, status: &str) -> SessionState {
        SessionState {
            id: id.into(),
            project: "p".into(),
            cwd: cwd.into(),
            status: status.into(),
            last_output: String::new(),
            tool_count: 0,
            recent_tools: Vec::new(),
            tty: String::new(),
            started_at: String::new(),
            modified_at: modified_at.into(),
            provider: "claude".into(),
        }
    }

    #[test]
    fn merge_keeps_newest_session_per_id_regardless_of_order() {
        // Regression: claude --resume creates a new JSONL file with the same
        // sessionId. Last-write-wins HashMap insert masks the active resume
        // with an older file. Merge must keep the newest.
        let s_old = fixture_session("abc", "/old", "2026-05-01T00:00:00+00:00", "idle");
        let s_new = fixture_session("abc", "/new", "2026-05-01T05:00:00+00:00", "waiting");

        let mut map = HashMap::new();
        merge_sessions_by_newest(&mut map, vec![s_old.clone(), s_new.clone()]);
        assert_eq!(map.len(), 1, "duplicate ids must collapse to one entry");
        assert_eq!(map.get("abc").unwrap().cwd, "/new");
        assert_eq!(map.get("abc").unwrap().status, "waiting");

        let mut map2 = HashMap::new();
        merge_sessions_by_newest(&mut map2, vec![s_new, s_old]);
        assert_eq!(map2.len(), 1);
        assert_eq!(
            map2.get("abc").unwrap().cwd,
            "/new",
            "newest must win regardless of input order"
        );
    }

    #[test]
    fn merge_keeps_distinct_ids() {
        let s1 = fixture_session("a", "/p1", "2026-05-01T00:00:00+00:00", "idle");
        let s2 = fixture_session("b", "/p2", "2026-05-01T00:00:00+00:00", "idle");
        let mut map = HashMap::new();
        merge_sessions_by_newest(&mut map, vec![s1, s2]);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn merge_keeps_existing_when_newer() {
        // map already has the newest entry; scanned brings older one. Keep existing.
        let mut map = HashMap::new();
        map.insert(
            "abc".into(),
            fixture_session("abc", "/new", "2026-05-01T05:00:00+00:00", "waiting"),
        );
        let s_old = fixture_session("abc", "/old", "2026-05-01T00:00:00+00:00", "idle");
        merge_sessions_by_newest(&mut map, vec![s_old]);
        assert_eq!(map.get("abc").unwrap().cwd, "/new");
        assert_eq!(map.get("abc").unwrap().status, "waiting");
    }

    #[test]
    fn rule_ids_are_unique() {
        let mut mgr = make_manager();
        // Reset counter to a known state isn't possible (static), but two
        // successive adds must produce different IDs.
        let id_a = {
            let n = RULE_COUNTER.fetch_add(1, Ordering::SeqCst);
            format!("rule-{}", n)
        };
        let id_b = {
            let n = RULE_COUNTER.fetch_add(1, Ordering::SeqCst);
            format!("rule-{}", n)
        };
        assert_ne!(id_a, id_b, "sequential rule IDs must be unique");

        mgr.add_rule(PipeRule {
            id: id_a.clone(),
            from_session_id: "s1".into(),
            to_session_id: "s2".into(),
            from_cwd: "/tmp/s1".into(),
            to_cwd: "/tmp/s2".into(),
            trigger: PipeTrigger::OnIdle,
        });
        mgr.add_rule(PipeRule {
            id: id_b.clone(),
            from_session_id: "s1".into(), // same pair
            to_session_id: "s2".into(),
            from_cwd: "/tmp/s1".into(),
            to_cwd: "/tmp/s2".into(),
            trigger: PipeTrigger::OnFileWrite("*.json".into()),
        });
        assert_eq!(mgr.rules.len(), 2, "two rules on same pair must both be stored");
    }

    #[test]
    fn add_and_remove_rule() {
        let mut mgr = make_manager();
        let id = "rule-test-remove".to_string();
        mgr.add_rule(PipeRule {
            id: id.clone(),
            from_session_id: "a".into(),
            to_session_id: "b".into(),
            from_cwd: "/tmp/a".into(),
            to_cwd: "/tmp/b".into(),
            trigger: PipeTrigger::OnIdle,
        });
        assert_eq!(mgr.rules.len(), 1);
        mgr.remove_rule(&id);
        assert!(mgr.rules.is_empty(), "rule must be removed by id");
    }

    #[test]
    fn remove_nonexistent_rule_is_noop() {
        let mut mgr = make_manager();
        mgr.remove_rule("does-not-exist"); // must not panic
        assert!(mgr.rules.is_empty());
    }

    #[test]
    fn list_rules_reflects_current_state() {
        let mut mgr = make_manager();
        assert!(mgr.rules.is_empty());
        mgr.add_rule(PipeRule {
            id: "r-list-1".into(),
            from_session_id: "x".into(),
            to_session_id: "y".into(),
            from_cwd: "/tmp/x".into(),
            to_cwd: "/tmp/y".into(),
            trigger: PipeTrigger::OnIdle,
        });
        mgr.add_rule(PipeRule {
            id: "r-list-2".into(),
            from_session_id: "x".into(),
            to_session_id: "z".into(),
            from_cwd: "/tmp/x".into(),
            to_cwd: "/tmp/z".into(),
            trigger: PipeTrigger::OnFileWrite("*.ts".into()),
        });
        assert_eq!(mgr.rules.len(), 2);
        mgr.remove_rule("r-list-1");
        assert_eq!(mgr.rules.len(), 1);
        assert_eq!(mgr.rules[0].id, "r-list-2");
    }
}
