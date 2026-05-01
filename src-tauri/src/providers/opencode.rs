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

    fn state_dir() -> Option<PathBuf> {
        dirs::data_dir().map(|d| d.join("opencode"))
    }

    fn db_path() -> Option<PathBuf> {
        Self::state_dir().map(|d| d.join("opencode.db"))
    }

    fn open_db() -> Option<Connection> {
        let path = Self::db_path()?;
        if !path.exists() {
            // opencode not installed or never run — quiet path, not an error.
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
            let status = compute_status(age_secs, time_archived);

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
/// `running`  — session updated in the last 10s
/// `idle`     — older than 10s, not archived
/// `dead`     — archived (time_archived IS NOT NULL)
///
/// TODO(v0.6.0): once a real opencode session has been captured with auth, add
/// `waiting` detection from the `event` table type vocabulary.
fn compute_status(age_secs: i64, time_archived: Option<i64>) -> String {
    if time_archived.is_some() {
        return "dead".to_string();
    }
    if age_secs < 10 {
        return "running".to_string();
    }
    "idle".to_string()
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

    #[test]
    fn test_compute_status_running() {
        assert_eq!(compute_status(0, None), "running");
        assert_eq!(compute_status(5, None), "running");
        assert_eq!(compute_status(9, None), "running");
    }

    #[test]
    fn test_compute_status_idle() {
        assert_eq!(compute_status(10, None), "idle");
        assert_eq!(compute_status(60, None), "idle");
        assert_eq!(compute_status(9999, None), "idle");
    }

    #[test]
    fn test_compute_status_dead_when_archived() {
        assert_eq!(compute_status(0, Some(1700000000000)), "dead");
        assert_eq!(compute_status(9999, Some(1700000000000)), "dead");
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
    fn test_ms_to_rfc3339_handles_known_epoch() {
        let s = ms_to_rfc3339(1714521600000);
        assert!(s.starts_with("2024-05-01"));
    }

    #[test]
    fn test_scan_sessions_returns_empty_when_db_missing() {
        let p = OpenCodeProvider::new();
        if OpenCodeProvider::db_path().map_or(true, |p| !p.exists()) {
            assert!(p.scan_sessions(Duration::from_secs(86400)).is_empty());
        }
    }
}
