//! Integration-style tests for IpcClient.
//!
//! These tests spin up a minimal Unix socket listener in-process to exercise
//! the full send/receive path without requiring a running humos-daemon.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use humos_client::IpcClient;
use humos_daemon::ipc::protocol::{Request, Response};

/// Bind a socket at a temp path, handle one connection with the given handler,
/// and return the path for the client to connect to.
fn mock_socket<F>(handler: F) -> std::path::PathBuf
where
    F: FnOnce(String) -> String + Send + 'static,
{
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.into_path().join("test.sock");
    let listener = UnixListener::bind(&path).expect("bind");
    let path_clone = path.clone();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut lines = reader.lines();
        if let Some(Ok(line)) = lines.next() {
            let response = handler(line);
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(b"\n");
            let _ = stream.flush();
        }
        // Keep dir alive by dropping here (path_clone keeps it referenced)
        drop(path_clone);
    });

    path
}

#[test]
fn ping_returns_pong() {
    let path = mock_socket(|_req| {
        // Respond with Pong regardless of request content
        serde_json::to_string(&Response::Pong).unwrap()
    });

    let client = IpcClient::new(path);
    assert!(client.ping().expect("ping should succeed"));
}

#[test]
fn health_returns_ok() {
    let path = mock_socket(|_req| {
        serde_json::to_string(&Response::Health {
            ok: true,
            index_sessions: 42,
            uptime_secs: 300,
        })
        .unwrap()
    });

    let client = IpcClient::new(path);
    let (ok, sessions, uptime) = client.health().expect("health should succeed");
    assert!(ok);
    assert_eq!(sessions, 42);
    assert_eq!(uptime, 300);
}

#[test]
fn call_enoent_returns_error() {
    let path = std::path::PathBuf::from("/tmp/humos-nonexistent-test.sock");
    let client = IpcClient::new(path);
    let result = client.call(&Request::Ping, Duration::from_millis(500));
    assert!(result.is_err(), "should error when socket does not exist");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("humos-daemon serve") || msg.contains("connect"),
        "error should mention daemon or connect: {msg}"
    );
}

#[test]
fn call_empty_response_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("empty.sock");
    let listener = UnixListener::bind(&path).expect("bind");
    let path_clone = path.clone();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        // Close without writing anything
        let _ = stream.write_all(b"\n"); // blank line
        drop(stream);
        drop(path_clone);
    });

    let client = IpcClient::new(path);
    let result = client.call(&Request::Ping, Duration::from_millis(500));
    assert!(result.is_err(), "empty response should return error");
}

#[test]
fn call_timeout_on_slow_socket() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("slow.sock");
    let listener = UnixListener::bind(&path).expect("bind");
    let path_clone = path.clone();

    thread::spawn(move || {
        let (_stream, _) = listener.accept().expect("accept");
        // Never respond — let the client time out
        thread::sleep(Duration::from_secs(10));
        drop(path_clone);
    });

    let client = IpcClient::new(path);
    let result = client.call(&Request::Ping, Duration::from_millis(100));
    // Should time out with a read error
    assert!(result.is_err(), "should time out");
}

#[test]
fn request_is_valid_json_sent_to_socket() {
    use std::sync::Mutex;
    let received: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let received_clone = Arc::clone(&received);

    let path = mock_socket(move |line| {
        *received_clone.lock().unwrap() = line;
        serde_json::to_string(&Response::Pong).unwrap()
    });

    let client = IpcClient::new(path);
    let _ = client.call(&Request::Ping, Duration::from_secs(1));

    let raw = received.lock().unwrap().clone();
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("request must be valid JSON");
    assert_eq!(
        parsed.get("type").and_then(|t| t.as_str()),
        Some("ping"),
        "Ping request must serialize with type=ping"
    );
}
