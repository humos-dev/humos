//! Stdio loop: read JSON-RPC requests one per line, dispatch, write responses.

use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};

use super::protocol::{
    ErrorObject, JsonRpcError, JsonRpcRequest, JsonRpcResponse, ToolCallResult, JSONRPC_VERSION,
    PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION,
};
use super::tools::{tool_definitions, ToolDispatcher};

pub fn run_stdio(dispatcher: Arc<ToolDispatcher>) -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log::warn!("stdin read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                write_response(
                    &mut stdout,
                    &JsonRpcError::new(
                        Value::Null,
                        super::protocol::ERR_PARSE,
                        format!("parse error: {e}"),
                        None,
                    ),
                )?;
                continue;
            }
        };

        if request.jsonrpc != JSONRPC_VERSION {
            write_response(
                &mut stdout,
                &JsonRpcError::new(
                    request.id.unwrap_or(Value::Null),
                    super::protocol::ERR_INVALID_REQUEST,
                    format!("unsupported jsonrpc version: {}", request.jsonrpc),
                    None,
                ),
            )?;
            continue;
        }

        // Notifications (no id) don't get responses.
        let id = match request.id {
            Some(id) => id,
            None => {
                // notifications/initialized is expected, no-op.
                continue;
            }
        };

        match request.method.as_str() {
            "initialize" => {
                let result = json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION
                    }
                });
                write_response(&mut stdout, &JsonRpcResponse::new(id, result))?;
            }
            "tools/list" => {
                let tools = tool_definitions();
                let result = json!({ "tools": tools });
                write_response(&mut stdout, &JsonRpcResponse::new(id, result))?;
            }
            "tools/call" => {
                let params = request.params.unwrap_or(Value::Null);
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(Value::Object(Default::default()));
                if name.is_empty() {
                    write_response(
                        &mut stdout,
                        &JsonRpcError::new(
                            id,
                            super::protocol::ERR_INVALID_PARAMS,
                            "tools/call missing 'name' parameter",
                            None,
                        ),
                    )?;
                    continue;
                }
                let result = match dispatcher.dispatch(name, &args) {
                    Ok(r) => r,
                    Err(e) => ToolCallResult::error_text(format!("tool dispatch failed: {e}")),
                };
                write_response(
                    &mut stdout,
                    &JsonRpcResponse::new(
                        id,
                        serde_json::to_value(result).unwrap_or_else(|_| Value::Null),
                    ),
                )?;
            }
            "ping" => {
                write_response(&mut stdout, &JsonRpcResponse::new(id, json!({})))?;
            }
            other => {
                write_response(
                    &mut stdout,
                    &JsonRpcError::new(
                        id,
                        super::protocol::ERR_METHOD_NOT_FOUND,
                        format!("unknown method: {other}"),
                        None,
                    ),
                )?;
            }
        }
    }

    Ok(())
}

fn write_response<T: serde::Serialize>(writer: &mut impl Write, response: &T) -> Result<()> {
    let mut s = serde_json::to_string(response)?;
    s.push('\n');
    writer.write_all(s.as_bytes())?;
    writer.flush()?;
    Ok(())
}

// Silence warning on ErrorObject if it ends up unused by a future refactor.
#[allow(dead_code)]
fn _keep_error_object(_e: ErrorObject) {}
