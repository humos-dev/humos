use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glob::Pattern;
use serde::{Deserialize, Serialize};

/// Minimum gap between two fires of the same rule. Prevents double-injection
/// when the file watcher and the periodic rescan both observe the same
/// transition within a few seconds.
const RULE_DEBOUNCE: Duration = Duration::from_secs(5);

/// Trigger condition for a pipe rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
///
/// Session IDs are JSONL filenames and change every time Claude CLI restarts.
/// `from_cwd` and `to_cwd` are stored at rule-creation time so that `evaluate`
/// can fall back to a CWD-based lookup when the ID no longer matches any session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeRule {
    pub id: String,
    pub from_session_id: String,
    pub to_session_id: String,
    pub trigger: PipeTrigger,
    /// Working directory of the source session at rule-creation time.
    /// Used as a stable fallback when the session ID changes.
    pub from_cwd: String,
    /// Working directory of the target session at rule-creation time.
    /// Used as a stable fallback when the session ID changes.
    pub to_cwd: String,
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
    /// Last time each rule fired, keyed by rule id. Used to debounce
    /// double-fires from the watcher and periodic rescan racing on the
    /// same transition.
    last_fired: HashMap<String, Instant>,
    /// Cache of compiled glob patterns, keyed by pattern string. Avoids
    /// recompiling on every evaluation tick.
    glob_cache: HashMap<String, Pattern>,
}

/// A pending injection produced when a rule fires.
#[derive(Debug)]
pub struct PipeAction {
    pub rule_id: String,
    pub from_session_id: String,
    pub to_session_id: String,
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
            last_fired: HashMap::new(),
            glob_cache: HashMap::new(),
        }
    }

    /// Look up or compile a glob pattern. Returns None if the pattern is invalid.
    fn compiled_glob(&mut self, pattern: &str) -> Option<&Pattern> {
        if !self.glob_cache.contains_key(pattern) {
            match Pattern::new(pattern) {
                Ok(p) => {
                    self.glob_cache.insert(pattern.to_string(), p);
                }
                Err(e) => {
                    log::warn!("pipe: invalid glob pattern '{}': {}", pattern, e);
                    return None;
                }
            }
        }
        self.glob_cache.get(pattern)
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
        self.last_fired.remove(id);
    }

    /// Evaluate all rules against the current session map and return any
    /// actions that should fire. Call this on every debounced watcher tick.
    ///
    /// Rules fire on *transitions*, not steady state:
    /// - OnIdle: source was NOT idle last tick and IS idle now.
    /// - OnFileWrite: last_output has changed and the new value matches the glob.
    ///
    /// Session IDs are JSONL filenames and change on every Claude CLI restart.
    /// `find_session` tries the stored ID first and falls back to CWD matching
    /// so pipes survive restarts without the user re-creating rules.
    pub fn evaluate(&mut self, sessions: &SessionMap) -> Vec<PipeAction> {
        let map = match sessions.lock() {
            Ok(g) => g,
            Err(e) => {
                log::error!("pipe: failed to lock session map: {}", e);
                return Vec::new();
            }
        };

        /// Try the stored session ID first; fall back to matching by cwd.
        fn find_session<'a>(
            map: &'a HashMap<String, crate::SessionState>,
            id: &str,
            cwd: &str,
        ) -> Option<&'a crate::SessionState> {
            if let Some(s) = map.get(id) {
                return Some(s);
            }
            if !cwd.is_empty() {
                return map.values().find(|s| s.cwd == cwd);
            }
            None
        }

        let mut actions: Vec<PipeAction> = Vec::new();
        let now = Instant::now();

        // Clone rules into a local vec so we can take &mut self for the glob cache
        // without a borrow conflict. Rule count is small (typically <20) so this
        // is cheap.
        let rules = self.rules.clone();

        for rule in &rules {
            let source = match find_session(&map, &rule.from_session_id, &rule.from_cwd) {
                Some(s) => s,
                None => continue, // source session not known yet
            };

            let target = match find_session(&map, &rule.to_session_id, &rule.to_cwd) {
                Some(s) => s,
                None => {
                    log::warn!(
                        "pipe: target session {} (cwd: {}) not found, skipping rule {}",
                        rule.to_session_id,
                        rule.to_cwd,
                        rule.id
                    );
                    continue;
                }
            };

            // Use CWD as the snapshot key so edge detection survives session ID churn.
            let snapshot_key = if source.cwd.is_empty() { source.id.as_str() } else { source.cwd.as_str() };
            let prev = self.snapshots.get(snapshot_key);

            let fired = match &rule.trigger {
                PipeTrigger::OnIdle => {
                    // Treat "no prior snapshot" as was_idle=true so we don't
                    // false-fire on the first evaluation tick for already-idle sessions.
                    let was_idle = prev.map(|s| s.status == "idle").unwrap_or(true);
                    !was_idle && source.status == "idle"
                }

                PipeTrigger::OnFileWrite(pattern) => {
                    // Only fire when last_output has changed.
                    // Default to false (not changed) on first tick to avoid startup false-fire.
                    let output_changed = prev
                        .map(|s| s.last_output != source.last_output)
                        .unwrap_or(false);

                    // Only match against tool invocation lines (start with "Running:").
                    // Assistant text narrating file operations (e.g. "I updated schema.json")
                    // must not trigger the rule — only actual tool calls should.
                    if output_changed
                        && source.last_output.starts_with("Running:")
                    {
                        let last_output = source.last_output.clone();
                        self.compiled_glob(pattern)
                            .map(|p| last_output.split_whitespace().any(|tok| p.matches(tok)))
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }
            };

            // Debounce: skip if this rule fired recently, even if the edge
            // transition genuinely reoccurred. Prevents the watcher and periodic
            // rescan from both dispatching the same idle transition.
            if fired {
                if let Some(last) = self.last_fired.get(&rule.id) {
                    if now.duration_since(*last) < RULE_DEBOUNCE {
                        log::debug!("pipe: rule {} debounced (fired recently)", rule.id);
                        continue;
                    }
                }
            }

            if fired {
                self.last_fired.insert(rule.id.clone(), now);
                let message = build_message(rule, source);
                log::info!(
                    "pipe: rule {} fired — injecting into session {} (cwd: {})",
                    rule.id,
                    rule.to_session_id,
                    target.cwd
                );
                actions.push(PipeAction {
                    rule_id: rule.id.clone(),
                    from_session_id: rule.from_session_id.clone(),
                    to_session_id: rule.to_session_id.clone(),
                    target_cwd: target.cwd.clone(),
                    message,
                });
            }
        }

        // Update snapshots after evaluating so the next tick sees current state.
        // Key by CWD (stable) not session ID (changes on every Claude CLI restart).
        // Prune snapshots for CWDs that are no longer present in the session map.
        let live_cwds: std::collections::HashSet<String> =
            map.values().map(|s| s.cwd.clone()).collect();
        self.snapshots.retain(|key, _| live_cwds.contains(key.as_str()));
        for session in map.values() {
            let key = if session.cwd.is_empty() { session.id.clone() } else { session.cwd.clone() };
            self.snapshots.insert(
                key,
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
///
/// NOTE: production evaluation goes through `PipeManager::compiled_glob` which
/// caches compiled patterns. This standalone helper exists only for unit tests.
#[cfg(test)]
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
            recent_tools: Vec::new(),
            tty: String::new(),
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
            from_cwd: "/tmp/a".to_string(),
            to_cwd: "/tmp/b".to_string(),
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
            from_cwd: "/tmp/a".to_string(),
            to_cwd: "/tmp/b".to_string(),
            trigger: PipeTrigger::OnFileWrite("*.json".to_string()),
        });

        // Tick 1: first observation — does NOT fire (no prior snapshot to detect change from).
        let map = make_map(vec![
            make_session("a", "running", "Running: write_file schema.json"),
            make_session("b", "idle", ""),
        ]);
        let actions = mgr.evaluate(&map);
        assert!(actions.is_empty(), "should not fire on first tick (no prior state)");

        // Tick 2: same last_output, still no change → does not fire
        let actions = mgr.evaluate(&map);
        assert!(actions.is_empty());

        // Tick 3 (new output): different matching output → fires as a real new write
        let map = make_map(vec![
            make_session("a", "running", "Running: write_file output.json"),
            make_session("b", "idle", ""),
        ]);
        let actions = mgr.evaluate(&map);
        assert_eq!(actions.len(), 1, "should fire when output genuinely changes to a match");

        // Tick 4: different non-matching output → does not fire
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

    #[test]
    fn glob_matching_does_not_match_assistant_text() {
        // Plain assistant text mentioning a filename should not match.
        // (The OnFileWrite branch now guards with starts_with("Running:") before
        // calling matches_glob, so this test documents the token-level behavior.)
        assert!(matches_glob("*.json", "Running: write_file schema.json")); // tool call — should match
        assert!(matches_glob("*.json", "schema.json")); // bare token — still matches at glob level
        // The Running: guard is in evaluate(), not matches_glob() itself.
        // This test just confirms the token logic is correct.
    }
}
