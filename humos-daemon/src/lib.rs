//! humos-daemon: background coordination runtime for humOS.
//!
//! Owns the tantivy session index at `~/.humOS/index/` and serves it to
//! humos-app and humos-mcp over a Unix socket at `~/.humOS/daemon.sock`.
//! This is Phase A of Plan 2. The MCP server, ribbon UI, and distribution
//! story land in subsequent PRs.

pub mod config;
pub mod doctor;
pub mod index;
pub mod ipc;
pub mod scanner;
