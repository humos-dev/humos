//! Unix socket IPC server. Newline-delimited JSON protocol.
//!
//! Protocol rationale: plain JSON lines instead of a binary protocol because
//! (1) easy to test with netcat, (2) MCP server in PR 2 also speaks JSON,
//! (3) we can inspect traffic in doctor / logs without tooling.

pub mod handler;
pub mod protocol;

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

use self::handler::Handler;
use self::protocol::{Request, Response};

pub struct IpcServer {
    listener: UnixListener,
    handler: Arc<Handler>,
    socket_path: std::path::PathBuf,
}

impl IpcServer {
    pub async fn bind(path: &Path, handler: Arc<Handler>) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create socket parent {}", parent.display()))?;
        }
        // Remove stale socket file if previous daemon crashed.
        if path.exists() {
            // Test if anything is listening before nuking.
            match UnixStream::connect(path).await {
                Ok(_) => {
                    anyhow::bail!(
                        "another humos-daemon appears to be running at {}",
                        path.display()
                    );
                }
                Err(_) => {
                    std::fs::remove_file(path)
                        .with_context(|| format!("remove stale socket {}", path.display()))?;
                }
            }
        }
        let listener = UnixListener::bind(path)
            .with_context(|| format!("bind unix socket {}", path.display()))?;
        log::info!("ipc listening on {}", path.display());
        Ok(Self {
            listener,
            handler,
            socket_path: path.to_path_buf(),
        })
    }

    pub async fn accept_loop(self) -> Result<()> {
        loop {
            match self.listener.accept().await {
                Ok((stream, _addr)) => {
                    let handler = Arc::clone(&self.handler);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, handler).await {
                            log::warn!("ipc connection closed: {e}");
                        }
                    });
                }
                Err(e) => {
                    log::error!("ipc accept failed: {e}");
                }
            }
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

async fn handle_connection(stream: UnixStream, handler: Arc<Handler>) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader).lines();
    let writer = Arc::new(Mutex::new(writer));

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err = Response::Error {
                    problem: "malformed request JSON".into(),
                    cause: e.to_string(),
                    fix: "send one Request object per line, newline-terminated".into(),
                    docs_url: Some(
                        "https://github.com/humos-dev/humos/blob/main/humos-daemon/README.md#ipc"
                            .into(),
                    ),
                };
                write_response(&writer, &err).await?;
                continue;
            }
        };
        let response = handler.dispatch(request).await;
        write_response(&writer, &response).await?;
    }
    Ok(())
}

async fn write_response(
    writer: &Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    response: &Response,
) -> Result<()> {
    let mut text = serde_json::to_string(response)?;
    text.push('\n');
    let mut w = writer.lock().await;
    w.write_all(text.as_bytes()).await?;
    w.flush().await?;
    Ok(())
}
