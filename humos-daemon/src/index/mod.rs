//! Session index: traits and types shared between keyword and future
//! embedding indexers. Phase A ships KeywordIndexer (tantivy-backed);
//! embedding-based implementation can plug into the same trait later.

pub mod keyword;
pub mod redact;
pub mod schema;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexableSession {
    pub id: String,
    pub provider: String,
    pub cwd: String,
    pub project: String,
    pub content: String,
    pub started_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub provider: String,
    pub cwd: String,
    pub project: String,
    pub snippet: String,
    pub modified_at: DateTime<Utc>,
    pub score: f32,
}

pub trait SessionIndexer: Send + Sync {
    fn index(&self, session: &IndexableSession) -> anyhow::Result<()>;
    fn delete(&self, session_id: &str) -> anyhow::Result<()>;
    fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>>;
    fn search_by_cwd(&self, cwd: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>>;
    fn total_count(&self) -> anyhow::Result<u64>;
    /// Force a commit (flush pending writes to disk).
    fn commit(&self) -> anyhow::Result<()>;
}

/// Truncate content to approximately `max_chars` characters at a UTF-8
/// boundary. Adds a trailing ellipsis when truncation occurred.
pub fn truncate_snippet(content: &str, max_chars: usize) -> String {
    let trimmed = content.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut end = 0usize;
    for (i, (byte_idx, _)) in trimmed.char_indices().enumerate() {
        if i == max_chars {
            end = byte_idx;
            break;
        }
        end = byte_idx;
    }
    format!("{}...", &trimmed[..end])
}
