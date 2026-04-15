# humos-mcp

MCP (Model Context Protocol) stdio server for humOS. Spawns under any
MCP-capable AI agent (Claude Code, Codex CLI, Cursor, etc.) and bridges
it to the humOS daemon, so the agent can search and recall past coding
sessions via tool calls.

## Tools exposed

| Tool | What it does |
|------|--------------|
| `search_sessions` | Keyword search across all indexed session content. Returns top N hits with snippet + cwd + provider. |
| `list_sessions` | List recent sessions, optionally filtered by `cwd`. |
| `get_project_context` | Return the most relevant past sessions for a given `cwd`. Call this at the start of a new session to recall what was worked on in the same repo. |
| `humos_health` | Report daemon status: online, indexed session count, uptime. |

## Prerequisites

`humos-daemon` must be running. Check with `humos-daemon doctor`.

## Build

```
cargo build --release -p humos-mcp
```

Binary lands at `target/release/humos-mcp`.

## Configure

### Claude Code (`~/.claude.json`)

```json
{
  "mcpServers": {
    "humos": {
      "command": "/Users/you/path/to/humos-mcp"
    }
  }
}
```

### Codex CLI (`~/.codex/config.toml`)

```toml
[[mcp_servers]]
name = "humos"
command = "/Users/you/path/to/humos-mcp"
```

### Cursor (`~/.cursor/mcp.json`)

```json
{
  "mcpServers": {
    "humos": {
      "command": "/Users/you/path/to/humos-mcp"
    }
  }
}
```

After configuring, restart your MCP client. The tools appear automatically.

## Debug

```
humos-mcp doctor
```

Runs 4 checks: socket exists, daemon reachable, tools surface, health probe.

```
humos-mcp version
```

## Protocol

JSON-RPC 2.0 over newline-delimited stdin/stdout. Implements `initialize`,
`tools/list`, `tools/call`, `ping`. All tool calls round-trip to the
daemon over its Unix socket at `~/.humOS/daemon.sock`.

Logs go to stderr. Stdout is reserved for the protocol.

## Status

PR 2 of 4 for Plan 2. PR 3 lights up the Project Brain ambient ribbon in
the humos-app using the same daemon. PR 4 ships distribution so all three
binaries (humos-app, humos-daemon, humos-mcp) install together.
