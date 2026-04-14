use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::Provider;
use crate::{applescript, parser, SessionState};

pub struct ClaudeProvider;

impl ClaudeProvider {
    pub fn new() -> Self {
        Self
    }

    fn claude_projects_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".claude").join("projects"))
    }

    /// Simple recursive directory walker returning file paths.
    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut result = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    result.extend(Self::walk(&path));
                } else {
                    result.push(path);
                }
            }
        }
        result
    }
}

impl Provider for ClaudeProvider {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        Self::claude_projects_dir().into_iter().collect()
    }

    fn owns_path(&self, path: &Path) -> bool {
        Self::claude_projects_dir()
            .map(|root| path.starts_with(&root) && path.extension().map_or(false, |e| e == "jsonl"))
            .unwrap_or(false)
    }

    fn parse_session(&self, path: &Path) -> Option<SessionState> {
        parser::parse_session_file(path)
    }

    fn scan_sessions(&self, max_age: Duration) -> Vec<SessionState> {
        let Some(root) = Self::claude_projects_dir() else {
            return Vec::new();
        };
        if !root.exists() {
            return Vec::new();
        }

        let cutoff = std::time::SystemTime::now().checked_sub(max_age);

        Self::walk(&root)
            .into_iter()
            .filter(|p| p.extension().map_or(false, |x| x == "jsonl"))
            .filter(|p| {
                if let (Some(cutoff), Ok(meta)) = (cutoff, p.metadata()) {
                    if let Ok(mtime) = meta.modified() {
                        return mtime >= cutoff;
                    }
                }
                true
            })
            .filter_map(|p| parser::parse_session_file(&p))
            .collect()
    }

    fn inject(&self, session: &SessionState, message: &str) -> Result<(), String> {
        if !session.tty.is_empty() {
            return applescript::inject_by_tty(&session.tty, message);
        }
        if !session.cwd.is_empty() {
            return applescript::inject_message(&session.cwd, message);
        }
        Err(format!("Session {} has no cwd or tty", session.id))
    }

    fn focus(&self, session: &SessionState) -> Result<(), String> {
        if !session.tty.is_empty() {
            return applescript::focus_terminal_by_tty(&session.tty);
        }
        if !session.cwd.is_empty() {
            return applescript::focus_terminal(&session.cwd);
        }
        Err(format!("Session {} has no cwd or tty", session.id))
    }

    fn broadcast(&self, message: &str) -> Result<usize, String> {
        applescript::broadcast_to_all_claude_tabs(message)
    }
}
