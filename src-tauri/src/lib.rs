mod applescript;
mod parser;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

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

/// Tauri command: return the last 50 raw JSONL lines for a session.
/// The frontend can pass this to the Claude API for summarization.
#[tauri::command]
fn summarize_session(session_id: String) -> Result<String, String> {
    let projects_dir = claude_projects_dir()
        .ok_or_else(|| "Cannot determine ~/.claude/projects directory".to_string())?;

    // Walk subdirectories to find the matching .jsonl file
    let jsonl_path = find_jsonl(&projects_dir, &session_id)
        .ok_or_else(|| format!("Session file not found for id: {}", session_id))?;

    let content = fs::read_to_string(&jsonl_path)
        .map_err(|e| format!("Failed to read session file: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();
    let last_50: Vec<&str> = lines.iter().rev().take(50).copied().collect::<Vec<_>>().into_iter().rev().collect();
    Ok(last_50.join("\n"))
}

/// Walk the projects directory and find a .jsonl whose stem matches session_id.
fn find_jsonl(base: &PathBuf, session_id: &str) -> Option<PathBuf> {
    for entry in walkdir_shallow(base) {
        let path = entry;
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if path.file_stem().and_then(|s| s.to_str()) == Some(session_id) {
                return Some(path);
            }
        }
    }
    None
}

/// Returns the ~/.claude/projects path.
fn claude_projects_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".claude").join("projects"))
}

/// Simple recursive directory walker returning file paths.
fn walkdir_shallow(dir: &PathBuf) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir_shallow(&path));
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

    let files = walkdir_shallow(&projects_dir);
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
fn start_watcher(app: AppHandle, sessions: SessionMap) {
    let Some(projects_dir) = claude_projects_dir() else {
        log::warn!("File watcher: could not determine ~/.claude/projects");
        return;
    };

    std::thread::spawn(move || {
        let sessions_clone = Arc::clone(&sessions);
        let app_clone = app.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(200),
            move |res: DebounceEventResult| {
                match res {
                    Ok(events) => {
                        for event in events {
                            for path in &event.paths {
                                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                                    continue;
                                }
                                if let Some(session) = parser::parse_session_file(path) {
                                    let id = session.id.clone();
                                    {
                                        let mut map = sessions_clone.lock().unwrap();
                                        map.insert(id.clone(), session.clone());
                                    }
                                    // Emit to frontend
                                    if let Err(e) =
                                        app_clone.emit("session-updated", &session)
                                    {
                                        log::error!("emit error: {}", e);
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

    // Initial scan
    scan_all_sessions(&sessions);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(sessions.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            start_watcher(handle, Arc::clone(&sessions));
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
