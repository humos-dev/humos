//! Wire types for the Unix socket protocol. Keep serialization stable.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::index::SearchResult;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Health,
    Search {
        query: String,
        #[serde(default = "default_limit")]
        limit: usize,
    },
    RelatedContext {
        cwd: String,
        #[serde(default = "default_limit")]
        limit: usize,
    },
    BulkRelatedContexts {
        cwds: Vec<String>,
        #[serde(default = "default_limit")]
        limit: usize,
    },
    Stats,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Pong,
    Health {
        ok: bool,
        index_sessions: u64,
        uptime_secs: u64,
    },
    SearchResults {
        results: Vec<SearchResult>,
    },
    RelatedContext {
        cwd: String,
        matches: Vec<SearchResult>,
        total_count: u64,
        recent_count: u64,
        is_stale: bool,
        daemon_online: bool,
    },
    BulkRelatedContexts {
        contexts: HashMap<String, BulkContextEntry>,
    },
    Stats {
        total_sessions: u64,
        index_dir: String,
        last_indexed_at: Option<DateTime<Utc>>,
    },
    Error {
        problem: String,
        cause: String,
        fix: String,
        docs_url: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BulkContextEntry {
    pub matches: Vec<SearchResult>,
    pub total_count: u64,
    pub recent_count: u64,
}

fn default_limit() -> usize {
    20
}
