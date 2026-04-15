//! Scanner: reuses ClaudeProvider from humos_lib to discover sessions,
//! converts them to IndexableSession, and feeds the index.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use humos_lib::providers::{claude::ClaudeProvider, Provider};
use humos_lib::SessionState;

use crate::config::HumosConfig;
use crate::index::{IndexableSession, SessionIndexer};

pub struct Scanner {
    indexer: Arc<dyn SessionIndexer>,
    providers: Vec<Box<dyn Provider>>,
    config: HumosConfig,
}

impl Scanner {
    pub fn new(indexer: Arc<dyn SessionIndexer>, config: HumosConfig) -> Self {
        let providers: Vec<Box<dyn Provider>> = vec![Box::new(ClaudeProvider::new())];
        Self {
            indexer,
            providers,
            config,
        }
    }

    /// Full scan. Call once at startup.
    pub fn scan_all(&self) -> Result<usize> {
        let max_age = Duration::from_secs(self.config.scan_days * 24 * 60 * 60);
        let mut indexed = 0usize;
        for provider in &self.providers {
            let sessions = provider.scan_sessions(max_age);
            for session in sessions {
                if !self.config.should_index(&session.cwd) {
                    continue;
                }
                let indexable = into_indexable(&session, provider.id());
                if let Err(e) = self.indexer.index(&indexable) {
                    log::warn!("index failed for {}: {}", indexable.id, e);
                } else {
                    indexed += 1;
                }
            }
        }
        self.indexer.commit()?;
        Ok(indexed)
    }

    /// Index a single file that just changed.
    pub fn index_path(&self, path: &std::path::Path) -> Result<bool> {
        for provider in &self.providers {
            if provider.owns_path(path) {
                if let Some(session) = provider.parse_session(path) {
                    if !self.config.should_index(&session.cwd) {
                        return Ok(false);
                    }
                    let indexable = into_indexable(&session, provider.id());
                    self.indexer.index(&indexable)?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn watch_paths(&self) -> Vec<std::path::PathBuf> {
        self.providers.iter().flat_map(|p| p.watch_paths()).collect()
    }
}

fn into_indexable(session: &SessionState, provider_id: &str) -> IndexableSession {
    let started_at = parse_timestamp(&session.started_at);
    let modified_at = parse_timestamp(&session.modified_at);

    let recent_tools = session.recent_tools.join(" ");
    let content = format!(
        "{}\n\nTools: {}\nProject: {}",
        session.last_output, recent_tools, session.project
    );

    IndexableSession {
        id: session.id.clone(),
        provider: provider_id.to_string(),
        cwd: session.cwd.clone(),
        project: session.project.clone(),
        content,
        started_at,
        modified_at,
    }
}

fn parse_timestamp(raw: &str) -> DateTime<Utc> {
    if raw.is_empty() {
        return Utc::now();
    }
    DateTime::parse_from_rfc3339(raw)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
