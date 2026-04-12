mod applescript;
mod parser;
pub mod pipe;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Global counter for unique rule IDs. Monotonically increasing, process-lifetime.
static RULE_COUNTER: AtomicU64 = AtomicU64::new(1);

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

/// The shared session state, keyed by session id.
type SessionMap = Arc<Mutex<HashMap<String, SessionState>>>;

/// Emitted on every status change (running → idle, etc.).
#[derive(serde::Serialize, Clone)]
struct SessionTransitioned {
    session_id: String,
    from_status: String,
    to_status: String,
}

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

/// Maximum age of session files to load during startup scan.
/// Files older than this are skipped to keep cold-start fast for users with
/// months of Claude CLI history.
const MAX_SESSION_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30 days

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

    // AppleScript calls are blocking (~200-500ms each). Offload the inject loop
    // to the blocking thread pool so we don't tie up a tokio worker for the
    // full broadcast duration.
    let msg_for_blocking = sanitized.clone();
    let targets_for_blocking = targets.clone();
    let results: Vec<SignalResult> = tokio::task::spawn_blocking(move || {
        let mut out = Vec::with_capacity(targets_for_blocking.len());
        for (id, project, cwd) in targets_for_blocking {
            let outcome = applescript::inject_message(&cwd, &msg_for_blocking);
            out.push(SignalResult {
                session_id: id,
                project,
                success: outcome.is_ok(),
                error: outcome.err(),
            });
        }
        out
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
    pub started_at: String,
    pub modified_at: String,
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

/// Tauri command: focus the Terminal window for a session.
#[tauri::command]
fn focus_session(session_id: String, state: State<'_, SessionMap>) -> Result<(), String> {
    let cwd = {
        let map = state.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&session_id).map(|s| s.cwd.clone())
    };

    match cwd {
        Some(cwd) if !cwd.is_empty() => applescript::focus_terminal(&cwd),
        _ => Err(format!("Session {} not found or has no cwd", session_id)),
    }
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
    // Validate message the same way signal_sessions does — empty strings,
    // over-length, and control characters all cause silent terminal breakage.
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

    // Try the session map first, fall back to the cwd the caller passed.
    let resolved_cwd = {
        let map = state.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&session_id)
            .map(|s| s.cwd.clone())
            .filter(|c| !c.is_empty())
            .or_else(|| cwd.filter(|c| !c.is_empty()))
    };

    match resolved_cwd {
        Some(cwd) => applescript::inject_message(&cwd, &sanitized),
        None => Err(format!(
            "Session {} not found. If Claude CLI was restarted, the session ID changed; humOS will recover on the next rescan.",
            session_id
        )),
    }
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

/// Scan all existing .jsonl files and load them into the session map.
/// Only files modified within `MAX_SESSION_AGE` are parsed to keep cold-start fast.
fn scan_all_sessions(sessions: &SessionMap) {
    let Some(projects_dir) = claude_projects_dir() else {
        log::warn!("Could not find ~/.claude/projects");
        return;
    };

    let cutoff = std::time::SystemTime::now()
        .checked_sub(MAX_SESSION_AGE)
        .unwrap_or(std::time::UNIX_EPOCH);

    let files = walkdir_recursive(&projects_dir);
    let mut map = sessions.lock().unwrap_or_else(|e| e.into_inner());
    let mut loaded: usize = 0;
    let mut skipped: usize = 0;
    for path in files {
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        // Skip files older than MAX_SESSION_AGE to avoid parsing thousands of
        // stale sessions on cold start.
        let mtime = path.metadata().ok().and_then(|m| m.modified().ok());
        if mtime.map(|m| m < cutoff).unwrap_or(true) {
            skipped += 1;
            continue;
        }
        if let Some(session) = parser::parse_session_file(&path) {
            map.insert(session.id.clone(), session);
            loaded += 1;
        }
    }
    log::info!("startup: scanned {} sessions ({} skipped as older than 30 days)", loaded, skipped);
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

/// Background thread: re-scan sessions modified in the last 60s and emit updates.
/// Runs every 5s to catch any files the notify watcher may have missed (large files,
/// rapid writes, bundled-app sandbox quirks).
fn start_periodic_rescan(app: AppHandle, sessions: SessionMap, pipe_manager: Arc<Mutex<pipe::PipeManager>>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let Some(projects_dir) = claude_projects_dir() else { continue };
            let files = walkdir_recursive(&projects_dir);
            let cutoff = std::time::SystemTime::now()
                .checked_sub(Duration::from_secs(60))
                .unwrap_or(std::time::UNIX_EPOCH);

            let mut any_updated = false;
            for path in files {
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                // Only reparse recently-modified files to keep CPU low.
                let mtime = path.metadata().ok().and_then(|m| m.modified().ok());
                if mtime.map(|m| m < cutoff).unwrap_or(true) {
                    continue;
                }
                if let Some(session) = parser::parse_session_file(&path) {
                    let mut map = sessions.lock().unwrap_or_else(|e| e.into_inner());
                    map.insert(session.id.clone(), session.clone());
                    drop(map);
                    let _ = app.emit("session-updated", &session);
                    any_updated = true;
                }
            }

            // Evaluate pipe rules after the rescan so OnIdle transitions that
            // the file watcher missed (e.g. large JSONL files debounced away)
            // are still caught and dispatched.
            if any_updated {
                let actions = pipe::evaluate_pipes(&pipe_manager, &sessions);
                for action in actions {
                    let inject_result = applescript::inject_message(&action.target_cwd, &action.message);
                    let (success, error_msg) = match &inject_result {
                        Ok(()) => (true, None),
                        Err(e) => {
                            log::error!("pipe(rescan): inject failed for {}: {}", action.target_cwd, e);
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
        }
    });
}

/// Start the notify file watcher in a background thread.
fn start_watcher(
    app: AppHandle,
    sessions: SessionMap,
    pipe_manager: Arc<Mutex<pipe::PipeManager>>,
) {
    let Some(projects_dir) = claude_projects_dir() else {
        log::warn!("File watcher: could not determine ~/.claude/projects");
        return;
    };

    std::thread::spawn(move || {
        let sessions_clone = Arc::clone(&sessions);
        let pipe_manager_clone = Arc::clone(&pipe_manager);
        let app_clone = app.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(200),
            move |res: DebounceEventResult| {
                match res {
                    Ok(events) => {
                        for event in events {
                            let path: &std::path::Path = event.path.as_path();
                            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                                continue;
                            }
                            if let Some(session) = parser::parse_session_file(path) {
                                let id = session.id.clone();
                                let old_state = {
                                    let mut map = sessions_clone.lock().unwrap_or_else(|e| e.into_inner());
                                    let old = map.get(&id).cloned();
                                    map.insert(id.clone(), session.clone());
                                    old
                                };
                                if let Err(e) = app_clone.emit("session-updated", &session) {
                                    log::error!("emit error: {}", e);
                                }
                                if let Some(old) = old_state {
                                    if old.status != session.status {
                                        let transition = SessionTransitioned {
                                            session_id: id.clone(),
                                            from_status: old.status.clone(),
                                            to_status: session.status.clone(),
                                        };
                                        if let Err(e) = app_clone.emit("session-transitioned", &transition) {
                                            log::error!("emit transition error: {}", e);
                                        }
                                    }
                                }
                                // Evaluate pipe rules and fire any triggered actions.
                                let actions = pipe::evaluate_pipes(&pipe_manager_clone, &sessions_clone);
                                for action in actions {
                                    let inject_result = applescript::inject_message(&action.target_cwd, &action.message);
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
                                    if let Err(e) = app_clone.emit("pipe-fired", &fired_evt) {
                                        log::error!("emit pipe-fired error: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => log::error!("watch error: {:?}", e),
                }
            },
        )
        .expect("Failed to create debouncer");

        debouncer
            .watcher()
            .watch(&projects_dir, RecursiveMode::Recursive)
            .expect("Failed to watch ~/.claude/projects");

        log::info!("File watcher started on {:?}", projects_dir);

        // Keep the thread alive
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let sessions: SessionMap = Arc::new(Mutex::new(HashMap::new()));
    let pipe_manager: Arc<Mutex<pipe::PipeManager>> = Arc::new(Mutex::new(pipe::PipeManager::new()));

    // Load persisted pipe rules before starting watchers.
    {
        let mut mgr = pipe_manager.lock().unwrap_or_else(|e| e.into_inner());
        load_pipe_rules(&mut mgr);
    }

    // Initial scan
    scan_all_sessions(&sessions);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(sessions.clone())
        .manage(pipe_manager.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            start_watcher(handle.clone(), Arc::clone(&sessions), Arc::clone(&pipe_manager));
            start_periodic_rescan(handle, Arc::clone(&sessions), Arc::clone(&pipe_manager));
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
