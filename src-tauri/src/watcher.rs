/// File watcher module — monitors ~/.claude/projects/**/*.jsonl
/// and emits Tauri events whenever a file changes.
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::parser;

/// Start watching the claude projects directory in a background thread.
/// Emits "sessions-updated" events to the frontend with fresh session data.
pub fn start_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let watch_dir = match get_watch_dir() {
            Some(d) => d,
            None => {
                eprintln!("[watcher] Could not resolve ~/.claude/projects — watcher not started");
                return;
            }
        };

        // Ensure the directory exists so we don't error on first launch
        if !watch_dir.exists() {
            std::fs::create_dir_all(&watch_dir).ok();
        }

        let (tx, rx) = mpsc::channel();

        let mut watcher: RecommendedWatcher = match Watcher::new(
            tx,
            Config::default().with_poll_interval(Duration::from_secs(2)),
        ) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[watcher] Failed to create watcher: {e}");
                return;
            }
        };

        if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::Recursive) {
            eprintln!("[watcher] Failed to watch {}: {e}", watch_dir.display());
            return;
        }

        println!("[watcher] Watching {}", watch_dir.display());

        // Emit an initial snapshot so the UI has data immediately on launch
        emit_sessions(&app, &watch_dir);

        for result in rx {
            match result {
                Ok(event) => {
                    let relevant = matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                    );
                    if relevant {
                        emit_sessions(&app, &watch_dir);
                    }
                }
                Err(e) => eprintln!("[watcher] Watch error: {e}"),
            }
        }
    });
}

fn get_watch_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".claude").join("projects"))
}

fn emit_sessions(app: &AppHandle, watch_dir: &PathBuf) {
    match parser::scan_sessions(watch_dir) {
        Ok(sessions) => {
            if let Err(e) = app.emit("sessions-updated", &sessions) {
                eprintln!("[watcher] Failed to emit sessions: {e}");
            }
        }
        Err(e) => eprintln!("[watcher] Failed to scan sessions: {e}"),
    }
}
