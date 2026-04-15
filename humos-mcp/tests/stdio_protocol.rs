//! Integration test for the JSON-RPC stdio dispatch.
//!
//! We drive the server with a scripted input/output pair: send an
//! `initialize` request, a `tools/list`, and an unknown method, then
//! assert each response has the correct shape. Tool calls that require
//! the daemon aren't tested here (they need a real socket). The
//! dispatcher is constructed with a client pointing at a nonexistent
//! path so tool/call would fail fast, which is fine, we don't call it.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn bin_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.push("target");
    p.push("debug");
    p.push("humos-mcp");
    p
}

#[test]
fn initialize_and_tools_list_round_trip() {
    let bin = bin_path();
    if !bin.exists() {
        eprintln!("skipping: binary not built at {}", bin.display());
        return;
    }

    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn humos-mcp");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // initialize
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
    )
    .unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).expect("read init response");
    assert!(line.contains("\"id\":1"));
    assert!(line.contains("protocolVersion"));
    assert!(line.contains("humos-mcp"));

    // tools/list
    line.clear();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
    reader.read_line(&mut line).expect("read tools/list response");
    assert!(line.contains("\"id\":2"));
    assert!(line.contains("search_sessions"));
    assert!(line.contains("get_project_context"));

    // unknown method
    line.clear();
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":3,"method":"does/not/exist"}}"#
    )
    .unwrap();
    reader.read_line(&mut line).expect("read error response");
    assert!(line.contains("\"id\":3"));
    assert!(line.contains("-32601"));

    drop(stdin);
    // Give the process a moment to exit cleanly.
    thread::sleep(Duration::from_millis(50));
    let _ = child.kill();
    let _ = child.wait();
}
