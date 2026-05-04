use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags};

use super::Provider;
use crate::{applescript, SessionState};

pub struct OpenCodeProvider;

impl OpenCodeProvider {
    pub fn new() -> Self {
        Self
    }

    /// opencode uses XDG paths on every platform, including macOS.
    /// `dirs::data_dir()` would return `~/Library/Application Support`
    /// on macOS (Apple convention), which is wrong. Honor `XDG_DATA_HOME`
    /// if set, otherwise fall back to the XDG default `~/.local/share`.
    fn state_dir() -> Option<PathBuf> {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("opencode"));
            }
        }
        dirs::home_dir().map(|h| h.join(".local").join("share").join("opencode"))
    }

    fn db_path() -> Option<PathBuf> {
        Self::state_dir().map(|d| d.join("opencode.db"))
    }

    fn open_db() -> Option<Connection> {
        let path = Self::db_path()?;
        if !path.exists() {
            // opencode not installed or never run. Quiet path, not an error.
            return None;
        }
        match Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => Some(c),
            Err(e) => {
                log::warn!("opencode: failed to open db at {:?}: {}", path, e);
                None
            }
        }
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}

impl Provider for OpenCodeProvider {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn display_name(&self) -> &'static str {
        "opencode"
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        Self::state_dir().into_iter().collect()
    }

    fn owns_path(&self, path: &Path) -> bool {
        Self::state_dir()
            .map(|root| path.starts_with(&root))
            .unwrap_or(false)
    }

    /// opencode is sqlite-backed, so a single file change does not map to a
    /// single session. Return None and let scan_sessions handle full re-query
    /// on the periodic poll.
    fn parse_session(&self, _path: &Path) -> Option<SessionState> {
        None
    }

    fn scan_sessions(&self, max_age: Duration) -> Vec<SessionState> {
        let Some(conn) = Self::open_db() else {
            return Vec::new();
        };

        let cutoff_ms = Self::now_ms().saturating_sub(max_age.as_millis() as i64);

        let sql = "SELECT id, directory, title, time_created, time_updated, time_archived \
                   FROM session \
                   WHERE time_archived IS NULL AND time_updated >= ?1 \
                   ORDER BY time_updated DESC";

        let mut stmt = match conn.prepare(sql) {
            Ok(s) => s,
            Err(e) => {
                // Schema drift: opencode renamed/dropped a column we depend on.
                log::warn!("opencode: prepare failed (schema may have changed): {}", e);
                return Vec::new();
            }
        };

        let rows = stmt.query_map([cutoff_ms], |row| {
            let id: String = row.get(0)?;
            let directory: String = row.get(1)?;
            let title: String = row.get(2)?;
            let time_created: i64 = row.get(3)?;
            let time_updated: i64 = row.get(4)?;
            let time_archived: Option<i64> = row.get(5)?;
            Ok((id, directory, title, time_created, time_updated, time_archived))
        });

        let rows = match rows {
            Ok(r) => r,
            Err(e) => {
                log::warn!("opencode: query_map failed: {}", e);
                return Vec::new();
            }
        };

        let now_ms = Self::now_ms();
        let mut out = Vec::new();
        for r in rows.flatten() {
            let (id, cwd, title, time_created, time_updated, time_archived) = r;
            let age_secs = (now_ms - time_updated).max(0) / 1000;
            let status = compute_status(age_secs, time_archived, &conn, &id);

            let project = Path::new(&cwd)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| cwd.clone());

            out.push(SessionState {
                id,
                project,
                cwd,
                status,
                last_output: title,
                tool_count: 0,
                recent_tools: Vec::new(),
                tty: String::new(),
                started_at: ms_to_rfc3339(time_created),
                modified_at: ms_to_rfc3339(time_updated),
                provider: "opencode".to_string(),
            });
        }
        out
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
        applescript::broadcast_to_terminal_tabs_running("opencode", message)
    }
}

/// Map opencode session state to humOS status vocabulary.
///
/// `running`: session updated in the last 10s and not waiting for approval
/// `waiting`: session updated in the last 10s and latest event is step-start
///            (a tool call was initiated but not yet resolved by the user)
/// `idle`:    older than 10s, not archived
/// `dead`:    archived (time_archived IS NOT NULL)
///
/// Waiting detection works by querying the event table for the most recent
/// event type for this session. The event stream is:
///   step-start -> [tool pending in TUI] -> tool -> step-finish
/// If the most recent committed event is step-start with no step-finish after
/// it, the session is blocked waiting for the user to approve a tool call.
fn compute_status(
    age_secs: i64,
    time_archived: Option<i64>,
    conn: &rusqlite::Connection,
    session_id: &str,
) -> String {
    if time_archived.is_some() {
        return "dead".to_string();
    }
    if age_secs >= 10 {
        return "idle".to_string();
    }
    // Session is recent. Check whether it is waiting for tool approval.
    // Query the event table for the highest-seq event on this session's aggregate.
    let latest_event_type: Option<String> = conn
        .query_row(
            "SELECT type FROM event WHERE aggregate_id = ?1 ORDER BY seq DESC LIMIT 1",
            [session_id],
            |row| row.get(0),
        )
        .ok();

    match latest_event_type.as_deref() {
        Some("step-start") => "waiting".to_string(),
        _ => "running".to_string(),
    }
}

fn ms_to_rfc3339(ms: i64) -> String {
    use chrono::{DateTime, Utc};
    let secs = ms / 1000;
    let nsec = ((ms % 1000) * 1_000_000) as u32;
    DateTime::<Utc>::from_timestamp(secs, nsec)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_event_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(
            "CREATE TABLE event (
                id           TEXT PRIMARY KEY,
                aggregate_id TEXT NOT NULL,
                seq          INTEGER NOT NULL,
                type         TEXT NOT NULL,
                data         TEXT
            );",
        )
        .expect("create event table");
        conn
    }

    fn insert_event(conn: &rusqlite::Connection, aggregate_id: &str, seq: i64, event_type: &str) {
        conn.execute(
            "INSERT INTO event (id, aggregate_id, seq, type) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                format!("evt-{}-{}", aggregate_id, seq),
                aggregate_id,
                seq,
                event_type
            ],
        )
        .expect("insert event");
    }

    #[test]
    fn test_compute_status_running() {
        let conn = empty_event_db();
        // No events for this session: defaults to running when recent
        assert_eq!(compute_status(0, None, &conn, "sess-1"), "running");
        assert_eq!(compute_status(5, None, &conn, "sess-1"), "running");
        assert_eq!(compute_status(9, None, &conn, "sess-1"), "running");
    }

    #[test]
    fn test_compute_status_waiting_when_step_start_is_latest() {
        let conn = empty_event_db();
        insert_event(&conn, "sess-w", 1, "step-start");
        insert_event(&conn, "sess-w", 2, "tool");
        insert_event(&conn, "sess-w", 3, "step-finish");
        insert_event(&conn, "sess-w", 4, "step-start"); // new step began, no finish yet
        assert_eq!(compute_status(5, None, &conn, "sess-w"), "waiting");
    }

    #[test]
    fn test_compute_status_running_when_step_finish_is_latest() {
        let conn = empty_event_db();
        insert_event(&conn, "sess-r", 1, "step-start");
        insert_event(&conn, "sess-r", 2, "step-finish");
        assert_eq!(compute_status(5, None, &conn, "sess-r"), "running");
    }

    #[test]
    fn test_compute_status_idle() {
        let conn = empty_event_db();
        // Age gate fires before event table is queried
        assert_eq!(compute_status(10, None, &conn, "sess-1"), "idle");
        assert_eq!(compute_status(60, None, &conn, "sess-1"), "idle");
        assert_eq!(compute_status(9999, None, &conn, "sess-1"), "idle");
    }

    #[test]
    fn test_compute_status_dead_when_archived() {
        let conn = empty_event_db();
        assert_eq!(compute_status(0, Some(1700000000000), &conn, "sess-1"), "dead");
        assert_eq!(compute_status(9999, Some(1700000000000), &conn, "sess-1"), "dead");
    }

    #[test]
    fn test_provider_identity() {
        let p = OpenCodeProvider::new();
        assert_eq!(p.id(), "opencode");
        assert_eq!(p.display_name(), "opencode");
    }

    #[test]
    fn test_owns_path_under_state_dir() {
        let p = OpenCodeProvider::new();
        if let Some(root) = OpenCodeProvider::state_dir() {
            let inside = root.join("opencode.db");
            assert!(p.owns_path(&inside));
            let outside = PathBuf::from("/tmp/random");
            assert!(!p.owns_path(&outside));
        }
    }

    #[test]
    fn test_state_dir_is_xdg_path_not_apple() {
        // Regression: on macOS, dirs::data_dir() returns
        // ~/Library/Application Support but opencode writes to
        // ~/.local/share. The path must end with .local/share/opencode.
        std::env::remove_var("XDG_DATA_HOME");
        let dir = OpenCodeProvider::state_dir().unwrap();
        let s = dir.to_string_lossy();
        assert!(
            s.ends_with(".local/share/opencode"),
            "expected XDG path ending in .local/share/opencode, got {}",
            s
        );
        assert!(
            !s.contains("Library/Application Support"),
            "must not use macOS Application Support path, got {}",
            s
        );
    }

    #[test]
    fn test_state_dir_honors_xdg_data_home() {
        std::env::set_var("XDG_DATA_HOME", "/tmp/custom-xdg");
        let dir = OpenCodeProvider::state_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/custom-xdg/opencode"));
        std::env::remove_var("XDG_DATA_HOME");
    }

    #[test]
    fn test_ms_to_rfc3339_handles_known_epoch() {
        let s = ms_to_rfc3339(1714521600000);
        assert!(s.starts_with("2024-05-01"));
    }

    // `test_scan_sessions_returns_empty_when_db_missing` was removed because
    // it raced with the two tests above that mutate `XDG_DATA_HOME`. Cargo
    // runs tests in parallel by default; one test setting the env var while
    // another reads it (twice, separated by a sqlite open) caused a torn
    // state where the assertion preconditions and the function under test
    // disagreed about which path to use.
    //
    // The graceful "DB missing returns empty" behavior is exercised
    // indirectly: `open_db` returns None when the file does not exist, and
    // `scan_sessions` returns `Vec::new()` from the early-return on None.
}
