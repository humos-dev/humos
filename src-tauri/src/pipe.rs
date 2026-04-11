use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use glob::Pattern;

/// Trigger condition for a pipe rule.
#[derive(Debug, Clone)]
pub enum PipeTrigger {
    /// Fire when the source session transitions to "idle".
    OnIdle,
    /// Fire when the source session's last_output matches a glob pattern
    /// (e.g. "*.json", "*.schema.ts"). The pattern is matched against the
    /// last_output string which contains file paths when the session writes
    /// files via tool_use.
    OnFileWrite(String),
}

/// A rule that connects two sessions: when `from_session_id` satisfies
/// `trigger`, inject a message into `to_session_id`'s terminal.
#[derive(Debug, Clone)]
pub struct PipeRule {
    pub id: String,
    pub from_session_id: String,
    pub to_session_id: String,
    pub trigger: PipeTrigger,
}

/// Snapshot of the previous session state used to detect edge transitions.
#[derive(Debug, Clone)]
struct SessionSnapshot {
    status: String,
    last_output: String,
}

/// Manages a set of pipe rules and tracks session state snapshots so rules
/// fire once per transition rather than on every poll tick.
pub struct PipeManager {
    pub rules: Vec<PipeRule>,
    /// Previous observed state for each session_id. Used to detect the
    /// idle-entry edge (running/waiting → idle) and new file-write events.
    snapshots: HashMap<String, SessionSnapshot>,
}

/// A pending injection produced when a rule fires.
#[derive(Debug)]
pub struct PipeAction {
    /// cwd of the target session (used by applescript::inject_message).
    pub target_cwd: String,
    /// The message to inject into the target terminal.
    pub message: String,
}

/// The shared session map type, matching lib.rs.
pub type SessionMap = Arc<Mutex<HashMap<String, crate::SessionState>>>;

impl PipeManager {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            snapshots: HashMap::new(),
        }
    }

    /// Add a rule. Returns the rule id on success.
    pub fn add_rule(&mut self, rule: PipeRule) -> &str {
        self.rules.push(rule);
        // Safety: we just pushed, so last() is always Some.
        &self.rules.last().unwrap().id
    }

    /// Remove a rule by id.
    pub fn remove_rule(&mut self, id: &str) {
        self.rules.retain(|r| r.id != id);
    }

    /// Evaluate all rules against the current session map and return any
    /// actions that should fire. Call this on every debounced watcher tick.
    ///
    /// Rules fire on *transitions*, not steady state:
    /// - OnIdle: source was NOT idle last tick and IS idle now.
    /// - OnFileWrite: last_output has changed and the new value matches the glob.
    pub fn evaluate(&mut self, sessions: &SessionMap) -> Vec<PipeAction> {
        let map = match sessions.lock() {
            Ok(g) => g,
            Err(e) => {
                log::error!("pipe: failed to lock session map: {}", e);
                return Vec::new();
            }
        };

        let mut actions: Vec<PipeAction> = Vec::new();

        for rule in &self.rules {
            let source = match map.get(&rule.from_session_id) {
                Some(s) => s,
                None => continue, // source session not known yet
            };

            let target = match map.get(&rule.to_session_id) {
                Some(s) => s,
                None => {
                    log::warn!(
                        "pipe: target session {} not found, skipping rule {}",
                        rule.to_session_id,
                        rule.id
                    );
                    continue;
                }
            };

            let prev = self.snapshots.get(&rule.from_session_id);

            let fired = match &rule.trigger {
                PipeTrigger::OnIdle => {
                    let was_idle = prev.map(|s| s.status == "idle").unwrap_or(false);
                    !was_idle && source.status == "idle"
                }

                PipeTrigger::OnFileWrite(pattern) => {
                    // Only fire when last_output has changed.
                    let output_changed = prev
                        .map(|s| s.last_output != source.last_output)
                        .unwrap_or(true);

                    if output_changed && !source.last_output.is_empty() {
                        matches_glob(pattern, &source.last_output)
                    } else {
                        false
                    }
                }
            };

            if fired {
                let message = build_message(rule, source);
                log::info!(
                    "pipe: rule {} fired — injecting into session {} (cwd: {})",
                    rule.id,
                    rule.to_session_id,
                    target.cwd
                );
                actions.push(PipeAction {
                    target_cwd: target.cwd.clone(),
                    message,
                });
            }
        }

        // Update snapshots after evaluating so the next tick sees current state.
        for (id, session) in map.iter() {
            self.snapshots.insert(
                id.clone(),
                SessionSnapshot {
                    status: session.status.clone(),
                    last_output: session.last_output.clone(),
                },
            );
        }

        actions
    }
}

impl Default for PipeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the message that will be injected into the target terminal.
fn build_message(rule: &PipeRule, source: &crate::SessionState) -> String {
    match &rule.trigger {
        PipeTrigger::OnIdle => {
            format!(
                "Session '{}' is idle. Last output: {}",
                source.project, source.last_output
            )
        }
        PipeTrigger::OnFileWrite(pattern) => {
            // Extract the file path from last_output. The parser stores
            // "Running: <tool_name>" for tool_use lines and the raw assistant
            // text for text lines, so last_output is the best proxy we have
            // until the parser tracks written file paths explicitly.
            format!(
                "File matching '{}' written by session '{}': {}",
                pattern, source.project, source.last_output
            )
        }
    }
}

/// Check whether `text` contains a substring that matches the glob `pattern`.
/// We test every whitespace-delimited token so "Running: write_file schema.json"
/// matches the pattern "*.json".
fn matches_glob(pattern: &str, text: &str) -> bool {
    let compiled = match Pattern::new(pattern) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("pipe: invalid glob pattern '{}': {}", pattern, e);
            return false;
        }
    };
    text.split_whitespace().any(|token| compiled.matches(token))
}

/// Convenience wrapper used directly in the watcher callback (lib.rs).
/// Accepts the shared PipeManager behind a Mutex so it can live in Tauri state.
pub fn evaluate_pipes(
    manager: &Arc<Mutex<PipeManager>>,
    sessions: &SessionMap,
) -> Vec<PipeAction> {
    match manager.lock() {
        Ok(mut mgr) => mgr.evaluate(sessions),
        Err(e) => {
            log::error!("pipe: failed to lock PipeManager: {}", e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionState;

    fn make_session(id: &str, status: &str, last_output: &str) -> SessionState {
        SessionState {
            id: id.to_string(),
            project: id.to_string(),
            cwd: format!("/tmp/{}", id),
            status: status.to_string(),
            last_output: last_output.to_string(),
            tool_count: 0,
            started_at: String::new(),
            modified_at: String::new(),
        }
    }

    fn make_map(sessions: Vec<SessionState>) -> SessionMap {
        let mut map = HashMap::new();
        for s in sessions {
            map.insert(s.id.clone(), s);
        }
        Arc::new(Mutex::new(map))
    }

    #[test]
    fn on_idle_fires_on_transition() {
        let mut mgr = PipeManager::new();
        mgr.add_rule(PipeRule {
            id: "r1".to_string(),
            from_session_id: "a".to_string(),
            to_session_id: "b".to_string(),
            trigger: PipeTrigger::OnIdle,
        });

        // First tick: a is running → no action, snapshot records "running"
        let map = make_map(vec![
            make_session("a", "running", ""),
            make_session("b", "idle", ""),
        ]);
        let actions = mgr.evaluate(&map);
        assert!(actions.is_empty(), "should not fire while running");

        // Second tick: a transitions to idle → rule fires
        let map = make_map(vec![
            make_session("a", "idle", "done"),
            make_session("b", "idle", ""),
        ]);
        let actions = mgr.evaluate(&map);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].target_cwd, "/tmp/b");

        // Third tick: a stays idle → no second fire (not a new transition)
        let actions = mgr.evaluate(&map);
        assert!(actions.is_empty(), "should not re-fire on same-state tick");
    }

    #[test]
    fn on_file_write_matches_glob() {
        let mut mgr = PipeManager::new();
        mgr.add_rule(PipeRule {
            id: "r2".to_string(),
            from_session_id: "a".to_string(),
            to_session_id: "b".to_string(),
            trigger: PipeTrigger::OnFileWrite("*.json".to_string()),
        });

        // Tick 1: last_output contains a .json path → fires on first observation
        let map = make_map(vec![
            make_session("a", "running", "Running: write_file schema.json"),
            make_session("b", "idle", ""),
        ]);
        let actions = mgr.evaluate(&map);
        assert_eq!(actions.len(), 1);

        // Tick 2: same last_output, no change → does not re-fire
        let actions = mgr.evaluate(&map);
        assert!(actions.is_empty());

        // Tick 3: different non-matching output → does not fire
        let map = make_map(vec![
            make_session("a", "running", "Running: read_file README.md"),
            make_session("b", "idle", ""),
        ]);
        let actions = mgr.evaluate(&map);
        assert!(actions.is_empty());
    }

    #[test]
    fn glob_matching_works() {
        assert!(matches_glob("*.json", "schema.json"));
        assert!(matches_glob("*.json", "Running: write schema.json"));
        assert!(!matches_glob("*.json", "schema.ts"));
        assert!(matches_glob("*.schema.*", "api.schema.ts"));
    }
}
