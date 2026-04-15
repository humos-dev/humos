//! Doctor: diagnose common MCP setup issues.

use std::time::Duration;

use humos_daemon::ipc::protocol::{Request, Response};

use crate::ipc_client::IpcClient;

pub fn run() -> anyhow::Result<()> {
    println!("humos-mcp doctor\n");

    let sock = IpcClient::default_socket();
    let client = IpcClient::new(sock.clone());

    check("daemon socket exists", || {
        if sock.exists() {
            Ok(format!("OK ({})", sock.display()))
        } else {
            Err(anyhow::anyhow!(
                "{} not found. Start the daemon first: humos-daemon",
                sock.display()
            ))
        }
    });

    check("daemon reachable", || {
        let ok = client.ping()?;
        if ok {
            Ok("OK (daemon responded to ping)".into())
        } else {
            Err(anyhow::anyhow!(
                "daemon did not respond with Pong. Try restarting humos-daemon."
            ))
        }
    });

    check("tools surface", || {
        let tools = crate::mcp::tools::tool_definitions();
        Ok(format!(
            "OK ({} tools registered: {})",
            tools.len(),
            tools.iter().map(|t| t.name).collect::<Vec<_>>().join(", ")
        ))
    });

    check("health probe", || {
        match client.call(&Request::Health, Duration::from_secs(2))? {
            Response::Health {
                ok,
                index_sessions,
                uptime_secs,
            } => Ok(format!(
                "OK (ok={ok}, sessions={index_sessions}, uptime={uptime_secs}s)"
            )),
            other => Err(anyhow::anyhow!(
                "unexpected health response: {other:?}"
            )),
        }
    });

    println!("\ndone.");
    println!("\nTo configure an MCP client:");
    println!("  Claude Code:  add to ~/.claude.json (see humos-mcp/README.md)");
    println!("  Codex CLI:    add to ~/.codex/config.toml");
    println!("  Cursor:       add to ~/.cursor/mcp.json");

    Ok(())
}

fn check<F>(name: &str, f: F)
where
    F: FnOnce() -> anyhow::Result<String>,
{
    match f() {
        Ok(msg) => println!("  [pass] {name}: {msg}"),
        Err(e) => println!("  [FAIL] {name}: {e}"),
    }
}
