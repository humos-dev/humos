mod applescript;
mod parser;
pub mod pipe;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

/// The shared session state, keyed by session id.
type SessionMap = Arc<Mutex<HashMap<String, SessionState>>>;

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
    let map = state.lock().unwrap();
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
        let map = state.lock().unwrap();
        map.get(&session_id).map(|s| s.cwd.clone())
    };

    match cwd {
        Some(cwd) if !cwd.is_empty() => applescript::focus_terminal(&cwd),
        _ => Err(format!("Session {} not found or has no cwd", session_id)),
    }
}

/// Tauri command: inject a message into the terminal for a session.
#[tauri::command]
fn inject_message(
    session_id: String,
    message: String,
    state: State<'_, SessionMap>,
) -> Result<(), String> {
    let cwd = {
        let map = state.lock().unwrap();
        map.get(&session_id).map(|s| s.cwd.clone())
    };

    match cwd {
        Some(cwd) if !cwd.is_empty() => applescript::inject_message(&cwd, &message),
        _ => Err(format!("Session {} not found or has no cwd", session_id)),
    }
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
fn scan_all_sessions(sessions: &SessionMap) {
    let Some(projects_dir) = claude_projects_dir() else {
        log::warn!("Could not find ~/.claude/projects");
        return;
    };

    let files = walkdir_recursive(&projects_dir);
    let mut map = sessions.lock().unwrap();
    for path in files {
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(session) = parser::parse_session_file(&path) {
            map.insert(session.id.clone(), session);
        }
    }
    log::info!("Initial scan complete: {} sessions loaded", map.len());
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
                                    let mut map = sessions_clone.lock().unwrap();
                                    let old = map.get(&id).cloned();
                                    map.insert(id.clone(), session.clone());
                                    old
                                };
                                if let Err(e) = app_clone.emit("session-updated", &session) {
                                    log::error!("emit error: {}", e);
                                }
                                if let Some(old) = old_state {
                                    if old.status != session.status {
                                        #[derive(serde::Serialize, Clone)]
                                        struct Transition {
                                            session_id: String,
                                            from_status: String,
                                            to_status: String,
                                        }
                                        let transition = Transition {
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
                                    if let Err(e) = applescript::inject_message(&action.target_cwd, &action.message) {
                                        log::error!("pipe: inject failed for {}: {}", action.target_cwd, e);
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

    // Initial scan
    scan_all_sessions(&sessions);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(sessions.clone())
        .manage(pipe_manager.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            start_watcher(handle, Arc::clone(&sessions), Arc::clone(&pipe_manager));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sessions,
            focus_session,
            inject_message,
            summarize_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
