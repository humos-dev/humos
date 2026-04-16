//! humos-mcp: MCP stdio server that bridges any MCP-capable AI agent
//! (Claude Code, Codex CLI, Cursor, etc.) to the humOS daemon.
//!
//! Phase B of Plan 2. The daemon (PR 1) owns the session index. This
//! binary spawns under an MCP client, reads JSON-RPC requests on stdin,
//! forwards data queries to the daemon over its Unix socket, and writes
//! JSON-RPC responses on stdout.

pub mod doctor;
pub mod mcp;
