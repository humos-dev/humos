//! Dispatch an IPC Request to the index and build a Response.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Duration, Utc};
use parking_lot_like::RwLock;

use crate::index::SessionIndexer;

use super::protocol::{BulkContextEntry, Request, Response};

/// Tiny std::sync::RwLock alias shim so we don't pull parking_lot. Keeping
/// naming consistent makes later refactor easy.
#[allow(non_snake_case, dead_code)]
mod parking_lot_like {
    pub use std::sync::RwLock;
}

pub struct Handler {
    indexer: Arc<dyn SessionIndexer>,
    start_time: Instant,
    last_indexed: RwLock<Option<DateTime<Utc>>>,
    is_stale_flag: RwLock<bool>,
}

impl Handler {
    pub fn new(indexer: Arc<dyn SessionIndexer>) -> Self {
        Self {
            indexer,
            start_time: Instant::now(),
            last_indexed: RwLock::new(None),
            is_stale_flag: RwLock::new(false),
        }
    }

    pub fn mark_indexed(&self) {
        let mut guard = self.last_indexed.write().unwrap();
        *guard = Some(Utc::now());
    }

    pub fn set_stale(&self, stale: bool) {
        *self.is_stale_flag.write().unwrap() = stale;
    }

    fn is_stale(&self) -> bool {
        *self.is_stale_flag.read().unwrap()
    }

    pub async fn dispatch(&self, request: Request) -> Response {
        match request {
            Request::Ping => Response::Pong,
            Request::Health => self.health(),
            Request::Search { query, limit } => self.search(query, limit),
            Request::RelatedContext { cwd, limit } => self.related_context(cwd, limit),
            Request::BulkRelatedContexts { cwds, limit } => self.bulk(cwds, limit),
            Request::Stats => self.stats(),
        }
    }

    fn health(&self) -> Response {
        let index_sessions = self.indexer.total_count().unwrap_or(0);
        let uptime_secs = self.start_time.elapsed().as_secs();
        Response::Health {
            ok: true,
            index_sessions,
            uptime_secs,
        }
    }

    fn search(&self, query: String, limit: usize) -> Response {
        match self.indexer.search(&query, limit) {
            Ok(results) => Response::SearchResults { results },
            Err(e) => Response::Error {
                problem: "search failed".into(),
                cause: e.to_string(),
                fix: "check the query syntax; tantivy query parser rejects unbalanced quotes and special chars".into(),
                docs_url: None,
            },
        }
    }

    fn related_context(&self, cwd: String, limit: usize) -> Response {
        match self.indexer.search_by_cwd(&cwd, limit) {
            Ok(matches) => {
                let total_count = matches.len() as u64;
                let recent_count = recent_count(&matches);
                Response::RelatedContext {
                    cwd,
                    matches,
                    total_count,
                    recent_count,
                    is_stale: self.is_stale(),
                    daemon_online: true,
                }
            }
            Err(e) => Response::Error {
                problem: "related_context failed".into(),
                cause: e.to_string(),
                fix: "verify the cwd is an absolute path; daemon may need a restart if the index is corrupt".into(),
                docs_url: None,
            },
        }
    }

    fn bulk(&self, cwds: Vec<String>, limit: usize) -> Response {
        let mut contexts: HashMap<String, BulkContextEntry> = HashMap::new();
        for cwd in cwds {
            match self.indexer.search_by_cwd(&cwd, limit) {
                Ok(matches) => {
                    let total_count = matches.len() as u64;
                    let recent_count = recent_count(&matches);
                    contexts.insert(
                        cwd,
                        BulkContextEntry {
                            matches,
                            total_count,
                            recent_count,
                        },
                    );
                }
                Err(e) => {
                    log::warn!("bulk related_context failed for {cwd}: {e}");
                    contexts.insert(
                        cwd,
                        BulkContextEntry {
                            matches: Vec::new(),
                            total_count: 0,
                            recent_count: 0,
                        },
                    );
                }
            }
        }
        Response::BulkRelatedContexts { contexts }
    }

    fn stats(&self) -> Response {
        let total = self.indexer.total_count().unwrap_or(0);
        let last = *self.last_indexed.read().unwrap();
        Response::Stats {
            total_sessions: total,
            index_dir: "~/.humOS/index".into(),
            last_indexed_at: last,
        }
    }
}

fn recent_count(matches: &[crate::index::SearchResult]) -> u64 {
    let cutoff = Utc::now() - Duration::hours(24);
    matches.iter().filter(|r| r.modified_at >= cutoff).count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{keyword::KeywordIndexer, IndexableSession};
    use tempfile::TempDir;

    fn make_handler() -> (Handler, TempDir) {
        let dir = TempDir::new().unwrap();
        let indexer: Arc<dyn SessionIndexer> =
            Arc::new(KeywordIndexer::open(dir.path()).unwrap());
        indexer
            .index(&IndexableSession {
                id: "test-session".into(),
                provider: "claude".into(),
                cwd: "/tmp/proj".into(),
                project: "proj".into(),
                content: "testing the ipc handler dispatch flow".into(),
                started_at: Utc::now(),
                modified_at: Utc::now(),
            })
            .unwrap();
        (Handler::new(indexer), dir)
    }

    #[tokio::test]
    async fn ping_returns_pong() {
        let (h, _dir) = make_handler();
        let resp = h.dispatch(Request::Ping).await;
        assert!(matches!(resp, Response::Pong));
    }

    #[tokio::test]
    async fn health_reports_session_count() {
        let (h, _dir) = make_handler();
        let resp = h.dispatch(Request::Health).await;
        match resp {
            Response::Health { ok, index_sessions, .. } => {
                assert!(ok);
                assert_eq!(index_sessions, 1);
            }
            other => panic!("expected Health, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn search_returns_indexed_content() {
        let (h, _dir) = make_handler();
        let resp = h.dispatch(Request::Search {
            query: "dispatch".into(),
            limit: 10,
        })
        .await;
        match resp {
            Response::SearchResults { results } => {
                assert!(!results.is_empty());
                assert_eq!(results[0].id, "test-session");
            }
            other => panic!("expected SearchResults, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn related_context_by_cwd() {
        let (h, _dir) = make_handler();
        let resp = h.dispatch(Request::RelatedContext {
            cwd: "/tmp/proj".into(),
            limit: 10,
        })
        .await;
        match resp {
            Response::RelatedContext { matches, total_count, daemon_online, .. } => {
                assert_eq!(total_count, 1);
                assert_eq!(matches.len(), 1);
                assert!(daemon_online);
            }
            other => panic!("expected RelatedContext, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn bulk_related_contexts_returns_entry_per_cwd() {
        let (h, _dir) = make_handler();
        let resp = h.dispatch(Request::BulkRelatedContexts {
            cwds: vec!["/tmp/proj".into(), "/nonexistent".into()],
            limit: 10,
        })
        .await;
        match resp {
            Response::BulkRelatedContexts { contexts } => {
                assert_eq!(contexts.len(), 2);
                let proj = contexts.get("/tmp/proj").unwrap();
                assert_eq!(proj.total_count, 1);
                let missing = contexts.get("/nonexistent").unwrap();
                assert_eq!(missing.total_count, 0);
            }
            other => panic!("expected BulkRelatedContexts, got {other:?}"),
        }
    }
}
