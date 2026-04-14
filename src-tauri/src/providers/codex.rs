use std::path::{Path, PathBuf};
use std::time::Duration;

use super::Provider;
use crate::SessionState;

/// Stub for Codex CLI support. Will be fleshed out once we confirm
/// where Codex stores session state locally. For now: reports no
/// sessions, registers as a known provider so SessionState.provider
/// can be "codex" without errors.
pub struct CodexProvider;

impl CodexProvider {
    pub fn new() -> Self {
        Self
    }

    fn codex_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".codex"))
    }
}

impl Provider for CodexProvider {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "OpenAI Codex CLI"
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        // TODO: add real watch paths once Codex session format is confirmed
        Self::codex_dir().filter(|p| p.exists()).into_iter().collect()
    }

    fn owns_path(&self, path: &Path) -> bool {
        Self::codex_dir().map(|root| path.starts_with(&root)).unwrap_or(false)
    }

    fn parse_session(&self, _path: &Path) -> Option<SessionState> {
        // TODO: implement once Codex session format is known
        None
    }

    fn scan_sessions(&self, _max_age: Duration) -> Vec<SessionState> {
        Vec::new()
    }

    fn inject(&self, _session: &SessionState, _message: &str) -> Result<(), String> {
        Err("Codex provider is a stub — inject not yet implemented".into())
    }

    fn focus(&self, _session: &SessionState) -> Result<(), String> {
        Err("Codex provider is a stub — focus not yet implemented".into())
    }

    fn broadcast(&self, _message: &str) -> Result<usize, String> {
        Ok(0)
    }
}
