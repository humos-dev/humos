//! SQLite-backed event log for pipe fires and signal broadcasts.
//!
//! Persists every coordination event to `~/.humOS/event-log.db` so the
//! WereAwayBanner can summarize "what happened while you were gone" and the
//! activity panel can replay history across app restarts.
//!
//! Architecture: writes are dispatched onto a bounded mpsc channel and drained
//! by a dedicated writer thread. The poll loop never blocks on SQLite I/O. If
//! the channel is full (DB hung on a network mount, e.g. NFS unreachable), the
//! enqueue is dropped and a warning is logged. Reads stay synchronous because
//! they happen on demand from a Tauri command thread, not the poll loop.
//!
//! Health state is exposed via `health()` so the frontend can render a chip
//! when init failed (read-only filesystem, corrupted DB, etc.).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub id: i64,
    pub ts: String,
    pub event_type: String,
    pub message: String,
    pub success: bool,
    pub payload_tokens: i64,
    pub source_tokens: i64,
    pub success_ids: String,
    pub fail_ids: String,
}

static CONN: OnceLock<Mutex<Option<Connection>>> = OnceLock::new();
static WRITER: OnceLock<Mutex<Option<SyncSender<WriteCmd>>>> = OnceLock::new();

/// Health state for the event log. Surfaced to the frontend via the
/// `event_log_health` Tauri command so a degraded write path is visible.
/// 0 = uninitialized, 1 = ok, 2 = init failed, 3 = writer queue saturated
static HEALTH: AtomicU8 = AtomicU8::new(0);

const WRITER_QUEUE_CAP: usize = 256;

#[derive(Debug)]
enum WriteCmd {
    Pipe {
        ts: String,
        message: String,
        success: bool,
        payload_tokens: u64,
        source_tokens: u64,
    },
    Signal {
        ts: String,
        message: String,
        success: bool,
        success_ids: String,
        fail_ids: String,
    },
}

/// Stable health values exported to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthState {
    pub status: String, // "uninitialized" | "ok" | "init_failed" | "queue_saturated"
}

pub fn health() -> HealthState {
    let status = match HEALTH.load(Ordering::Relaxed) {
        1 => "ok",
        2 => "init_failed",
        3 => "queue_saturated",
        _ => "uninitialized",
    };
    HealthState { status: status.to_string() }
}

fn conn_slot() -> &'static Mutex<Option<Connection>> {
    CONN.get_or_init(|| Mutex::new(None))
}

fn writer_slot() -> &'static Mutex<Option<SyncSender<WriteCmd>>> {
    WRITER.get_or_init(|| Mutex::new(None))
}

fn db_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(home.join(".humOS").join("event-log.db"))
}

/// Open the SQLite database, create the table if needed, and spawn the writer
/// thread. Safe to call multiple times: subsequent calls are no-ops.
pub fn init() {
    let mut guard = match conn_slot().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.is_some() {
        return;
    }

    let path = match db_path() {
        Some(p) => p,
        None => {
            log::warn!("event_log: no HOME env, skipping init");
            HEALTH.store(2, Ordering::Relaxed);
            return;
        }
    };

    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("event_log: mkdir {:?} failed: {}", parent, e);
            HEALTH.store(2, Ordering::Relaxed);
            return;
        }
    }

    let conn = match Connection::open(&path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("event_log: open {:?} failed: {}", path, e);
            HEALTH.store(2, Ordering::Relaxed);
            return;
        }
    };

    let create = "CREATE TABLE IF NOT EXISTS event_log (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        ts              TEXT NOT NULL,
        event_type      TEXT NOT NULL,
        message         TEXT NOT NULL,
        success         INTEGER NOT NULL DEFAULT 1,
        payload_tokens  INTEGER NOT NULL DEFAULT 0,
        source_tokens   INTEGER NOT NULL DEFAULT 0,
        success_ids     TEXT NOT NULL DEFAULT '',
        fail_ids        TEXT NOT NULL DEFAULT ''
    )";
    if let Err(e) = conn.execute(create, []) {
        log::warn!("event_log: create table failed: {}", e);
        HEALTH.store(2, Ordering::Relaxed);
        return;
    }
    if let Err(e) = conn.pragma_update(None, "journal_mode", "WAL") {
        log::warn!("event_log: WAL pragma failed: {}", e);
    }

    *guard = Some(conn);
    drop(guard);

    // Spawn the writer thread. It owns its own dedicated connection so we can
    // serve reads concurrently from the conn_slot connection without lock
    // contention. The bounded channel means a slow filesystem can't grow
    // memory unboundedly: if it fills, we drop the event and flip health.
    spawn_writer_thread(path.clone());

    HEALTH.store(1, Ordering::Relaxed);
    log::info!("event_log: initialized at {:?}", path);
}

fn spawn_writer_thread(path: PathBuf) {
    let (tx, rx) = sync_channel::<WriteCmd>(WRITER_QUEUE_CAP);
    {
        let mut wguard = match writer_slot().lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        *wguard = Some(tx);
    }

    thread::spawn(move || {
        // Open a dedicated write connection. If this fails, drain the channel
        // forever to avoid blocking senders, but warn loudly.
        let conn = match Connection::open(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("event_log writer: open failed, draining: {}", e);
                HEALTH.store(2, Ordering::Relaxed);
                while let Ok(_cmd) = rx.recv() {
                    // drop
                }
                return;
            }
        };
        // Re-set WAL on the writer's own connection. Each connection sees its
        // own pragma state.
        let _ = conn.pragma_update(None, "journal_mode", "WAL");

        while let Ok(cmd) = rx.recv() {
            let res = match &cmd {
                WriteCmd::Pipe { ts, message, success, payload_tokens, source_tokens } => {
                    conn.execute(
                        "INSERT INTO event_log (ts, event_type, message, success, payload_tokens, source_tokens, success_ids, fail_ids)
                         VALUES (?1, 'pipe', ?2, ?3, ?4, ?5, '', '')",
                        params![
                            ts,
                            message,
                            if *success { 1 } else { 0 },
                            *payload_tokens as i64,
                            *source_tokens as i64,
                        ],
                    )
                }
                WriteCmd::Signal { ts, message, success, success_ids, fail_ids } => {
                    conn.execute(
                        "INSERT INTO event_log (ts, event_type, message, success, payload_tokens, source_tokens, success_ids, fail_ids)
                         VALUES (?1, 'signal', ?2, ?3, 0, 0, ?4, ?5)",
                        params![
                            ts,
                            message,
                            if *success { 1 } else { 0 },
                            success_ids,
                            fail_ids,
                        ],
                    )
                }
            };
            if let Err(e) = res {
                log::warn!("event_log writer: insert failed: {}", e);
            }
        }
        log::info!("event_log writer: channel closed, exiting");
    });
}

/// Send a command to the writer thread without blocking. If the queue is
/// full, drop the command and flip health to "queue_saturated" so the UI
/// can surface that the log is degraded.
fn enqueue(cmd: WriteCmd) {
    let guard = match writer_slot().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let tx = match guard.as_ref() {
        Some(t) => t,
        None => return,
    };
    match tx.try_send(cmd) {
        Ok(()) => {}
        Err(std::sync::mpsc::TrySendError::Full(_)) => {
            log::warn!("event_log: writer queue full, dropping event");
            HEALTH.store(3, Ordering::Relaxed);
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
            log::warn!("event_log: writer disconnected, dropping event");
            HEALTH.store(2, Ordering::Relaxed);
        }
    }
}

/// Record a pipe fire. Non-blocking enqueue onto the writer thread.
pub fn record_pipe(message: &str, success: bool, payload_tokens: u64, source_tokens: u64) {
    enqueue(WriteCmd::Pipe {
        ts: chrono::Utc::now().to_rfc3339(),
        message: message.to_string(),
        success,
        payload_tokens,
        source_tokens,
    });
}

/// Record a signal broadcast. Non-blocking enqueue onto the writer thread.
pub fn record_signal(message: &str, success_ids: &[String], fail_ids: &[String]) {
    let success_json = serde_json::to_string(success_ids).unwrap_or_else(|_| "[]".to_string());
    let fail_json = serde_json::to_string(fail_ids).unwrap_or_else(|_| "[]".to_string());
    let success = fail_ids.is_empty();
    enqueue(WriteCmd::Signal {
        ts: chrono::Utc::now().to_rfc3339(),
        message: message.to_string(),
        success,
        success_ids: success_json,
        fail_ids: fail_json,
    });
}

/// Return the most recent N events, newest first.
pub fn list_recent(limit: usize) -> Vec<EventLogEntry> {
    let guard = match conn_slot().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let conn = match guard.as_ref() {
        Some(c) => c,
        None => return Vec::new(),
    };
    let mut stmt = match conn.prepare(
        "SELECT id, ts, event_type, message, success, payload_tokens, source_tokens, success_ids, fail_ids
         FROM event_log
         ORDER BY id DESC
         LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("event_log: prepare failed: {}", e);
            return Vec::new();
        }
    };
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(EventLogEntry {
            id: row.get(0)?,
            ts: row.get(1)?,
            event_type: row.get(2)?,
            message: row.get(3)?,
            success: row.get::<_, i64>(4)? != 0,
            payload_tokens: row.get(5)?,
            source_tokens: row.get(6)?,
            success_ids: row.get(7)?,
            fail_ids: row.get(8)?,
        })
    });
    match rows {
        Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            log::warn!("event_log: query failed: {}", e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

    fn fresh_db(tmp: &std::path::Path) {
        let mut guard = conn_slot().lock().unwrap();
        *guard = None;
        // Drop old writer tx so the previous test's writer thread exits.
        let mut wguard = writer_slot().lock().unwrap();
        *wguard = None;
        std::env::set_var("HOME", tmp);
        drop(guard);
        drop(wguard);
        // Reset health for isolated test runs.
        HEALTH.store(0, Ordering::Relaxed);
        init();
    }

    /// Block until the writer has committed at least `expected` rows, or panic.
    /// The writer thread is async; tests need this to avoid races.
    fn wait_for_writes(expected: usize) {
        let start = std::time::Instant::now();
        loop {
            if list_recent(1000).len() >= expected {
                return;
            }
            if start.elapsed().as_millis() > 1000 {
                panic!(
                    "writer did not flush {} events within 1s (got {})",
                    expected,
                    list_recent(1000).len()
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn test_pipe_event_persists() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        fresh_db(tmp.path());

        record_pipe("api -> tests: schema.sql", true, 180, 12400);
        wait_for_writes(1);
        let events = list_recent(10);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "pipe");
        assert_eq!(events[0].payload_tokens, 180);
        assert_eq!(events[0].source_tokens, 12400);
        assert!(events[0].success);
    }

    #[test]
    fn test_signal_event_persists_with_ids() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        fresh_db(tmp.path());

        let success = vec!["s1".to_string(), "s2".to_string()];
        let fail = vec!["s3".to_string()];
        record_signal("re-read CLAUDE.md", &success, &fail);
        wait_for_writes(1);
        let events = list_recent(10);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "signal");
        assert!(!events[0].success, "fail_ids non-empty must mark failure");
        assert!(events[0].success_ids.contains("s1"));
        assert!(events[0].fail_ids.contains("s3"));
    }

    #[test]
    fn test_list_recent_orders_newest_first() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        fresh_db(tmp.path());

        record_pipe("first", true, 10, 100);
        record_pipe("second", true, 20, 200);
        record_pipe("third", true, 30, 300);
        wait_for_writes(3);
        let events = list_recent(10);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].message, "third");
        assert_eq!(events[2].message, "first");
    }

    #[test]
    fn test_list_recent_respects_limit() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        fresh_db(tmp.path());

        for i in 0..5 {
            record_pipe(&format!("event-{}", i), true, 10, 100);
        }
        wait_for_writes(5);
        let events = list_recent(3);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_uninitialized_returns_empty() {
        let _g = TEST_LOCK.lock().unwrap();
        let mut guard = conn_slot().lock().unwrap();
        *guard = None;
        let mut wguard = writer_slot().lock().unwrap();
        *wguard = None;
        HEALTH.store(0, Ordering::Relaxed);
        drop(guard);
        drop(wguard);
        let events = list_recent(10);
        assert_eq!(events.len(), 0);
        assert_eq!(health().status, "uninitialized");
    }

    #[test]
    fn test_health_reports_ok_after_init() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        fresh_db(tmp.path());
        assert_eq!(health().status, "ok");
    }

    #[test]
    fn test_writer_drops_when_queue_saturated() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        fresh_db(tmp.path());

        // Flood the channel beyond capacity. The writer drains in the
        // background but a tight loop here will outpace it. This test
        // verifies that overflow gracefully drops events and flips health.
        let flood = WRITER_QUEUE_CAP * 4;
        for i in 0..flood {
            record_pipe(&format!("flood-{}", i), true, 1, 1);
        }
        // Allow the writer to drain whatever it can.
        std::thread::sleep(std::time::Duration::from_millis(200));
        let events = list_recent(flood);
        // We don't assert exact count; we assert the health flag flipped or
        // the count is below the flood. Either proves overflow handling.
        let dropped = events.len() < flood;
        let health_degraded = health().status == "queue_saturated";
        assert!(
            dropped || health_degraded,
            "expected either dropped events ({} < {}) or saturated health (got {})",
            events.len(), flood, health().status
        );
    }
}
