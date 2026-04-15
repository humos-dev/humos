//! Minimal MCP JSON-RPC 2.0 types.
//!
//! We hand-roll this instead of pulling an MCP SDK so we stay in control
//! of the wire format. The protocol surface we need is small: initialize,
//! tools/list, tools/call.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const JSONRPC_VERSION: &str = "2.0";
pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "humos-mcp";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A JSON-RPC request. `id` is omitted on notifications.
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    pub id: Option<Value>,
}

/// A successful JSON-RPC response.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub result: Value,
}

impl JsonRpcResponse {
    pub fn new(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            result,
        }
    }
}

/// A JSON-RPC error response.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub error: ErrorObject,
}

#[derive(Debug, Serialize)]
pub struct ErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn new(id: Value, code: i32, message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            error: ErrorObject {
                code,
                message: message.into(),
                data,
            },
        }
    }
}

// Standard JSON-RPC error codes.
pub const ERR_PARSE: i32 = -32700;
pub const ERR_INVALID_REQUEST: i32 = -32600;
pub const ERR_METHOD_NOT_FOUND: i32 = -32601;
pub const ERR_INVALID_PARAMS: i32 = -32602;
#[allow(dead_code)]
pub const ERR_INTERNAL: i32 = -32603;

/// Tool definition returned by tools/list.
#[derive(Debug, Serialize)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// A single chunk of tool output. MCP supports text, image, resource.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    Text { text: String },
}

/// tools/call result envelope.
#[derive(Debug, Serialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl ToolCallResult {
    pub fn text(body: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: body.into() }],
            is_error: false,
        }
    }

    pub fn error_text(body: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: body.into() }],
            is_error: true,
        }
    }
}
