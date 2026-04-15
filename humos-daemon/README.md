# humos-daemon

Background coordination runtime for humOS. Owns the tantivy session index
and serves it to `humos-app` and `humos-mcp` over a Unix socket.

## Build

```
cargo build --release -p humos-daemon
```

Binary lands at `target/release/humos-daemon`.

## Run

```
humos-daemon              # default: runs the daemon
humos-daemon doctor       # health check
humos-daemon version
```

Logs go to stderr. Use `RUST_LOG=debug` for verbose output.

## Paths

- Index: `~/.humOS/index/`
- Socket: `~/.humOS/daemon.sock`
- Config: `~/.humOS/config.toml` (optional)

## Config

All fields optional. Defaults shown.

```toml
# ~/.humOS/config.toml

# Exact prefixes to exclude (no indexing, no search).
exclude_cwds = []

# Glob patterns matched against cwd.
exclude_patterns = []

# Skip Project Brain auto-context injection (PR 3).
disable_project_brain = false

# Days of history to index at startup.
scan_days = 7

# Overrides (usually leave as defaults).
# index_path = "/Users/bolu/.humOS/index"
# socket_path = "/Users/bolu/.humOS/daemon.sock"
```

## Secret redaction

Common credential patterns (API keys, bearer tokens, private key blocks)
are regex-scrubbed before content enters the index. Set
`HUMOS_INDEX_REDACT=off` to disable for debugging.

This is a defense-in-depth layer, not a full DLP solution. The raw JSONL
session files are still on disk and an attacker with filesystem access
can read them directly.

## IPC protocol

Newline-delimited JSON. Each request goes on one line, each response on
one line. Connect with netcat:

```
nc -U ~/.humOS/daemon.sock
{"type":"ping"}
```

Response:
```
{"type":"pong"}
```

Requests:

- `{"type":"ping"}`
- `{"type":"health"}`
- `{"type":"search","query":"auth flow","limit":10}`
- `{"type":"related_context","cwd":"/path","limit":5}`
- `{"type":"bulk_related_contexts","cwds":["/a","/b"],"limit":5}`
- `{"type":"stats"}`

Errors return `{"type":"error","problem":"...","cause":"...","fix":"...","docs_url":null}`.

## Status

This is Phase A of Plan 2. Phase B adds an MCP stdio server that speaks
the same IPC protocol under the hood. Phase C migrates `humos-app` onto
this daemon and lights up the Project Brain ribbon. Phase D ships a
Homebrew tap and launchd plist for auto-start.
