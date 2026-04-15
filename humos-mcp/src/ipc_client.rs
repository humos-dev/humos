//! Thin client over the daemon's Unix socket IPC.
//!
//! One connection per tool call. Connections are cheap and short-lived.
//! Keeping a persistent connection would require reconnect-on-error logic
//! which is out of scope for the MCP phase.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use humos_daemon::ipc::protocol::{Request, Response};

pub struct IpcClient {
    socket_path: PathBuf,
}

impl IpcClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub fn default_socket() -> PathBuf {
        dirs::home_dir()
            .expect("home directory")
            .join(".humOS")
            .join("daemon.sock")
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Send a request and return the single response.
    /// Blocks up to `timeout` on read.
    pub fn call(&self, request: &Request, timeout: Duration) -> Result<Response> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .with_context(|| format!("connect to humos-daemon at {}", self.socket_path.display()))?;
        stream.set_read_timeout(Some(timeout)).ok();
        stream.set_write_timeout(Some(timeout)).ok();

        let mut payload = serde_json::to_string(request).context("serialize request")?;
        payload.push('\n');
        stream.write_all(payload.as_bytes()).context("write request")?;
        stream.flush().ok();

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).context("read response line")?;
        if line.trim().is_empty() {
            anyhow::bail!("daemon returned empty response");
        }
        let response: Response =
            serde_json::from_str(line.trim()).context("parse response JSON")?;
        Ok(response)
    }

    pub fn ping(&self) -> Result<bool> {
        match self.call(&Request::Ping, Duration::from_secs(2))? {
            Response::Pong => Ok(true),
            _ => Ok(false),
        }
    }
}
