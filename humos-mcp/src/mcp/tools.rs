//! Tool registration and dispatch. Each tool is a thin wrapper around
//! a daemon IPC call.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use humos_daemon::ipc::protocol::{Request, Response};
use serde_json::{json, Value};

use super::protocol::{ToolCallResult, ToolDefinition};
use humos_client::IpcClient;

const TOOL_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_LIMIT: usize = 20;

pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "search_sessions",
            description: "Search past AI coding agent sessions by content. Returns the most relevant sessions with a truncated snippet, cwd, and provider.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Keywords to search session content for."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results to return. Default 20.",
                        "minimum": 1,
                        "maximum": 100
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "list_sessions",
            description: "List recent sessions for a working directory. If cwd is omitted, lists the most recent sessions across all cwds.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cwd": {
                        "type": "string",
                        "description": "Absolute directory path. Filters to sessions that ran in this tree."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results to return. Default 20.",
                        "minimum": 1,
                        "maximum": 100
                    }
                }
            }),
        },
        ToolDefinition {
            name: "get_project_context",
            description: "Get the most relevant past sessions for a given cwd. Use this at the start of a new session to recall what was previously worked on in the same repo.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cwd": {
                        "type": "string",
                        "description": "Absolute directory path of the current workspace."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max related sessions to return. Default 5.",
                        "minimum": 1,
                        "maximum": 50
                    }
                },
                "required": ["cwd"]
            }),
        },
        ToolDefinition {
            name: "humos_health",
            description: "Check humOS daemon health: reports whether the daemon is reachable, how many sessions are indexed, and uptime.",
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

pub struct ToolDispatcher {
    client: Arc<IpcClient>,
}

impl ToolDispatcher {
    pub fn new(client: Arc<IpcClient>) -> Self {
        Self { client }
    }

    pub fn dispatch(&self, name: &str, args: &Value) -> Result<ToolCallResult> {
        match name {
            "search_sessions" => self.search(args),
            "list_sessions" => self.list(args),
            "get_project_context" => self.project_context(args),
            "humos_health" => self.health(),
            other => Ok(ToolCallResult::error_text(format!(
                "Unknown tool: {other}. Call tools/list to see available tools."
            ))),
        }
    }

    fn search(&self, args: &Value) -> Result<ToolCallResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing 'query' string argument"))?
            .to_string();
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_LIMIT);

        match self.client.call(
            &Request::Search { query, limit },
            TOOL_TIMEOUT,
        )? {
            Response::SearchResults { results } => {
                Ok(ToolCallResult::text(format_search_results(&results, "No sessions matched the query.")))
            }
            Response::Error { problem, cause, fix, docs_url } => {
                Ok(ToolCallResult::error_text(format_daemon_error(&problem, &cause, &fix, docs_url.as_deref())))
            }
            other => Ok(ToolCallResult::error_text(format!(
                "Daemon returned unexpected response: {other:?}"
            ))),
        }
    }

    fn list(&self, args: &Value) -> Result<ToolCallResult> {
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_LIMIT);
        let cwd = args.get("cwd").and_then(|v| v.as_str()).map(String::from);

        let request = match cwd {
            Some(cwd) => Request::RelatedContext { cwd, limit },
            None => Request::Search {
                query: String::new(),
                limit,
            },
        };

        match self.client.call(&request, TOOL_TIMEOUT)? {
            Response::SearchResults { results } => {
                Ok(ToolCallResult::text(format_search_results(&results, "No recent sessions.")))
            }
            Response::RelatedContext { matches, total_count, recent_count, is_stale, .. } => {
                let header = format!(
                    "{total_count} session{}, {recent_count} in the last 24h{}:\n\n",
                    if total_count == 1 { "" } else { "s" },
                    if is_stale { " (index rebuilding)" } else { "" }
                );
                let body = format_search_results(&matches, "No sessions in that cwd.");
                Ok(ToolCallResult::text(format!("{header}{body}")))
            }
            Response::Error { problem, cause, fix, docs_url } => {
                Ok(ToolCallResult::error_text(format_daemon_error(&problem, &cause, &fix, docs_url.as_deref())))
            }
            other => Ok(ToolCallResult::error_text(format!(
                "Daemon returned unexpected response: {other:?}"
            ))),
        }
    }

    fn project_context(&self, args: &Value) -> Result<ToolCallResult> {
        let cwd = args
            .get("cwd")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing 'cwd' string argument"))?
            .to_string();
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(5);

        match self.client.call(
            &Request::RelatedContext { cwd: cwd.clone(), limit },
            TOOL_TIMEOUT,
        )? {
            Response::RelatedContext { matches, total_count, recent_count, is_stale, .. } => {
                if matches.is_empty() {
                    return Ok(ToolCallResult::text(format!(
                        "No past sessions found for {cwd}. Nothing to recall yet."
                    )));
                }
                let header = format!(
                    "Project Brain for {cwd}\n{total_count} related session{}, {recent_count} recent{}:\n\n",
                    if total_count == 1 { "" } else { "s" },
                    if is_stale { " (index rebuilding)" } else { "" }
                );
                let body = format_search_results(&matches, "No sessions.");
                Ok(ToolCallResult::text(format!("{header}{body}")))
            }
            Response::Error { problem, cause, fix, docs_url } => {
                Ok(ToolCallResult::error_text(format_daemon_error(&problem, &cause, &fix, docs_url.as_deref())))
            }
            other => Ok(ToolCallResult::error_text(format!(
                "Daemon returned unexpected response: {other:?}"
            ))),
        }
    }

    fn health(&self) -> Result<ToolCallResult> {
        match self.client.call(&Request::Health, Duration::from_secs(2))? {
            Response::Health { ok, index_sessions, uptime_secs } => Ok(ToolCallResult::text(format!(
                "humOS daemon: {}\nindexed sessions: {index_sessions}\nuptime: {uptime_secs}s\nsocket: {}",
                if ok { "online" } else { "degraded" },
                self.client.socket_path().display(),
            ))),
            other => Ok(ToolCallResult::error_text(format!(
                "Daemon returned unexpected response: {other:?}"
            ))),
        }
    }
}

fn format_search_results(
    results: &[humos_daemon::index::SearchResult],
    empty_msg: &str,
) -> String {
    if results.is_empty() {
        return empty_msg.to_string();
    }
    let mut out = String::new();
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!(
            "{}. [{}] {} ({}) {}\n   {}\n",
            i + 1,
            r.provider,
            r.project,
            r.modified_at.format("%Y-%m-%d %H:%M"),
            r.cwd,
            r.snippet.replace('\n', " ")
        ));
    }
    out
}

fn format_daemon_error(problem: &str, cause: &str, fix: &str, docs_url: Option<&str>) -> String {
    let mut msg = format!("Daemon error\nproblem: {problem}\ncause: {cause}\nfix: {fix}");
    if let Some(url) = docs_url {
        msg.push_str(&format!("\ndocs: {url}"));
    }
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use humos_daemon::index::SearchResult;
    use chrono::{TimeZone, Utc};

    #[test]
    fn tool_definitions_are_well_formed() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 4);
        let names: Vec<_> = tools.iter().map(|t| t.name).collect();
        assert!(names.contains(&"search_sessions"));
        assert!(names.contains(&"list_sessions"));
        assert!(names.contains(&"get_project_context"));
        assert!(names.contains(&"humos_health"));

        for tool in &tools {
            assert!(!tool.description.is_empty(), "{} missing description", tool.name);
            assert_eq!(tool.input_schema.get("type").and_then(|v| v.as_str()), Some("object"));
        }
    }

    #[test]
    fn format_search_results_empty_uses_fallback() {
        let out = format_search_results(&[], "nothing here");
        assert_eq!(out, "nothing here");
    }

    #[test]
    fn format_search_results_includes_provider_project_cwd_snippet() {
        let result = SearchResult {
            id: "s1".into(),
            provider: "claude".into(),
            cwd: "/Users/bolu/humos".into(),
            project: "humos".into(),
            snippet: "wired provider registry\ninto lib.rs".into(),
            modified_at: Utc.with_ymd_and_hms(2026, 4, 15, 12, 34, 0).unwrap(),
            score: 1.5,
        };
        let out = format_search_results(&[result], "nope");
        assert!(out.contains("[claude]"));
        assert!(out.contains("humos"));
        assert!(out.contains("/Users/bolu/humos"));
        assert!(out.contains("wired provider registry"));
        // newline in snippet should be flattened
        assert!(!out.contains("registry\ninto"));
    }

    #[test]
    fn daemon_error_format_has_all_fields() {
        let msg = format_daemon_error("search failed", "bad query", "check syntax", Some("https://docs"));
        assert!(msg.contains("problem: search failed"));
        assert!(msg.contains("cause: bad query"));
        assert!(msg.contains("fix: check syntax"));
        assert!(msg.contains("docs: https://docs"));
    }

    #[test]
    fn daemon_error_format_without_docs_url() {
        let msg = format_daemon_error("x", "y", "z", None);
        assert!(!msg.contains("docs:"));
    }
}
