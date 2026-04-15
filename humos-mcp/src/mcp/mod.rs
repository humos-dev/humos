//! MCP (Model Context Protocol) stdio server.
//!
//! Protocol: JSON-RPC 2.0 over newline-delimited stdin/stdout.
//! Spec: https://modelcontextprotocol.io/specification

pub mod protocol;
pub mod server;
pub mod tools;
