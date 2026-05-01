use std::path::{Path, PathBuf};
use std::time::Duration;
use crate::SessionState;

/// Every AI coding agent provider implements this trait.
/// Providers handle their own session discovery and message injection.
pub trait Provider: Send + Sync {
    /// Unique identifier: "claude", "codex", "cursor", etc.
    fn id(&self) -> &'static str;

    /// Human-readable name for the UI
    fn display_name(&self) -> &'static str;

    /// Root directories to watch for session files.
    /// Empty if this provider uses polling/webhooks instead.
    fn watch_paths(&self) -> Vec<PathBuf>;

    /// Decide if this provider owns a given path (called by file watcher)
    fn owns_path(&self, path: &Path) -> bool;

    /// Parse a single session file/directory into SessionState
    fn parse_session(&self, path: &Path) -> Option<SessionState>;

    /// Scan all existing sessions, filtering by max_age
    fn scan_sessions(&self, max_age: Duration) -> Vec<SessionState>;

    /// Inject a message into a session's terminal/UI
    fn inject(&self, session: &SessionState, message: &str) -> Result<(), String>;

    /// Focus the window for this session
    fn focus(&self, session: &SessionState) -> Result<(), String>;

    /// Broadcast a message to ALL active sessions of this provider.
    /// Returns count of sessions that received the message.
    fn broadcast(&self, message: &str) -> Result<usize, String>;
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: Box<dyn Provider>) {
        self.providers.push(provider);
    }

    pub fn all_watch_paths(&self) -> Vec<PathBuf> {
        self.providers.iter().flat_map(|p| p.watch_paths()).collect()
    }

    pub fn parse_changed_file(&self, path: &Path) -> Option<SessionState> {
        self.providers
            .iter()
            .find(|p| p.owns_path(path))
            .and_then(|p| p.parse_session(path))
    }

    pub fn scan_all(&self, max_age: Duration) -> Vec<SessionState> {
        self.providers.iter().flat_map(|p| p.scan_sessions(max_age)).collect()
    }

    pub fn inject(&self, session: &SessionState, message: &str) -> Result<(), String> {
        self.provider_for(session)?.inject(session, message)
    }

    pub fn focus(&self, session: &SessionState) -> Result<(), String> {
        self.provider_for(session)?.focus(session)
    }

    pub fn broadcast(&self, message: &str) -> Result<usize, String> {
        let mut total = 0usize;
        let mut last_err: Option<String> = None;
        for p in &self.providers {
            match p.broadcast(message) {
                Ok(n) => total += n,
                Err(e) => {
                    log::warn!("broadcast to {} failed: {}", p.id(), e);
                    last_err = Some(e);
                }
            }
        }
        if total == 0 {
            Err(last_err.unwrap_or_else(|| "No active sessions.".into()))
        } else {
            Ok(total)
        }
    }

    fn provider_for(&self, session: &SessionState) -> Result<&dyn Provider, String> {
        self.providers
            .iter()
            .find(|p| p.id() == session.provider)
            .map(|p| p.as_ref())
            .ok_or_else(|| format!("Unknown provider: {}", session.provider))
    }
}

pub mod claude;
pub mod opencode;

#[cfg(test)]
mod tests {
    use super::*;
    use claude::ClaudeProvider;

    #[test]
    fn registry_registers_claude_provider() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(ClaudeProvider::new()));
        assert_eq!(registry.providers.len(), 1);
        assert_eq!(registry.providers[0].id(), "claude");
        assert_eq!(registry.providers[0].display_name(), "Claude Code");
    }

    #[test]
    fn registry_collects_watch_paths_from_claude() {
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(ClaudeProvider::new()));
        let paths = registry.all_watch_paths();
        assert!(paths.iter().any(|p| p.ends_with("projects")));
    }

    #[test]
    fn dispatch_for_unknown_provider_errors() {
        let registry = ProviderRegistry::new();
        let session = SessionState {
            id: "s".into(),
            project: "p".into(),
            cwd: "/tmp".into(),
            status: "idle".into(),
            last_output: String::new(),
            tool_count: 0,
            recent_tools: Vec::new(),
            tty: String::new(),
            started_at: String::new(),
            modified_at: String::new(),
            provider: "nonexistent".into(),
        };
        assert!(registry.inject(&session, "hi").is_err());
        assert!(registry.focus(&session).is_err());
    }

    #[test]
    fn empty_registry_broadcast_errors() {
        let registry = ProviderRegistry::new();
        assert!(registry.broadcast("hi").is_err());
    }
}
