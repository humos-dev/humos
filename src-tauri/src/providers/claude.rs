use std::path::{Path, PathBuf};
use std::time::Duration;

use super::Provider;
use crate::SessionState;

/// Placeholder. Real implementation lands in the next commit.
pub struct ClaudeProvider;

impl ClaudeProvider {
    pub fn new() -> Self { Self }
}

impl Provider for ClaudeProvider {
    fn id(&self) -> &'static str { "claude" }
    fn display_name(&self) -> &'static str { "Claude Code" }
    fn watch_paths(&self) -> Vec<PathBuf> { Vec::new() }
    fn owns_path(&self, _path: &Path) -> bool { false }
    fn parse_session(&self, _path: &Path) -> Option<SessionState> { None }
    fn scan_sessions(&self, _max_age: Duration) -> Vec<SessionState> { Vec::new() }
    fn inject(&self, _session: &SessionState, _message: &str) -> Result<(), String> {
        Err("ClaudeProvider not yet implemented".into())
    }
    fn focus(&self, _session: &SessionState) -> Result<(), String> {
        Err("ClaudeProvider not yet implemented".into())
    }
    fn broadcast(&self, _message: &str) -> Result<usize, String> { Ok(0) }
}
