# OpenCodeAdapter Spike — v0.6.0

Spike date: 2026-05-01
Target: humOS v0.6.0 cross-vendor coordination
Decision: substitute opencode for Codex CLI as the first non-Claude adapter

## Why opencode (not Codex CLI)

- Open source. Session schema is inspectable, not reverse-engineered.
- No API-key dependency for users. Works with any provider opencode supports (Anthropic, OpenAI, models via providers it auths into).
- Demo equivalence: a `Claude Code + opencode` side-by-side signal() broadcast carries the same cross-vendor weight as `Claude Code + Codex` for the launch story.
- Lower install friction for humOS testers. `brew install sst/tap/opencode` plus `opencode providers login` and a session exists.

## Install + footprint

| Item | Value |
|------|-------|
| Install | `brew install sst/tap/opencode` |
| Version tested | 1.14.30 |
| Binary | `/opt/homebrew/bin/opencode` |
| State dir | `~/.local/share/opencode/` |
| Config dir | `~/.config/opencode/` |
| Database | `~/.local/share/opencode/opencode.db` (sqlite + WAL) |
| Logs | `~/.local/share/opencode/log/` |

## Architecture vs. Claude Code

| Aspect | Claude Code | opencode |
|--------|-------------|----------|
| Persistence | append-only JSONL per session under `~/.claude/projects/` | sqlite database, drizzle-managed schema |
| Event model | derived from JSONL line types | first-class `event` table (event-sourced, `aggregate_id` + `seq`) |
| TUI/IPC | none (file is the API) | HTTP server (`opencode serve`), TUI, ACP (stdio) |
| Status detection | parse JSONL for tool-use boundaries | poll `event` table by recency + `time_updated` on session |
| Multi-machine | n/a | mDNS service discovery in `acp --mdns` |

## Schema (relevant tables)

```
project   (id, worktree, vcs, name, time_created, time_updated, ...)
session   (id, project_id, parent_id, slug, directory, title, version,
           time_created, time_updated, time_compacting, time_archived,
           workspace_id, summary_*)
message   (id, session_id, time_created, time_updated, data JSON)
part      (id, message_id, session_id, time_created, time_updated, data JSON)
event     (id, aggregate_id, seq, type, data JSON)
todo      (session_id, content, status, priority, position, ...)
workspace (id, type, name, branch, directory, project_id, ...)
```

Key fields for humOS integration:
- `session.directory` = working directory. Direct cwd match for the existing humOS session-by-cwd model.
- `session.time_updated` = wall-clock recency proxy.
- `session.time_compacting`, `session.time_archived` = lifecycle indicators.
- `event` table = event-sourced activity stream, the right surface for status detection.

## Three integration paths (evaluated)

### Path A — sqlite poller (RECOMMENDED for v0.6.0)

Open `~/.local/share/opencode/opencode.db` read-only in WAL mode. Poll periodically.

Pros:
- No dependency on a running opencode server. Works whenever opencode has been used.
- Same architectural shape as `ClaudeParser` watching JSONL files. Fits the existing humOS parser registry pattern.
- Persistent — captures sessions even if opencode TUI is closed.
- Schema is structured. No ad-hoc parsing, just SQL.

Cons:
- WAL contention if humOS polls during heavy opencode writes. Mitigated by read-only `?mode=ro&immutable=0` open + short-lived connections.
- Macos sqlite version may need bump in the workspace `Cargo.toml`.

Implementation:
- New crate `humos-opencode` or extend `humos-client` with an `OpenCodeReader`.
- Use `rusqlite` (already a candidate dep) with `OpenFlags::SQLITE_OPEN_READ_ONLY`.
- Poll every 2-5s. Map rows to `SessionState` via a new variant in the parser registry.
- Status derivation: `running` if `event.aggregate_id == session.id` had a row in the last 10s; `idle` otherwise; `waiting` from a specific event `type` (TBD — needs event capture from a real run).

### Path B — HTTP API via `opencode serve`

`opencode serve` exposes an OpenAPI-spec'd HTTP server. Verified surface in the unsecured (no `OPENCODE_SERVER_PASSWORD`) mode is minimal: `/auth/{providerID}`, `/log`. The richer endpoints (sessions, messages, events) appear to be auth-gated or require an active TUI session — not yet confirmed.

Pros:
- Structured API, future-proof if opencode expands the spec.
- mDNS support could let humOS coordinate sessions across machines.

Cons:
- Requires `opencode serve` running. Adds a process dependency users must manage.
- Spec surface in default mode is too narrow today. Would need upstream changes or auth wiring.

Verdict: defer to v0.7+ when the API stabilizes. Watch the spec.

### Path C — ACP (Agent Client Protocol)

ACP is stdio-based JSON-RPC. Designed for editors (Zed) to spawn an agent as a subprocess. Not the right shape for humOS, which observes pre-existing sessions rather than spawning them.

Verdict: not applicable for humOS adapter.

## Status mapping (proposed)

| humOS status | opencode signal |
|--------------|-----------------|
| `running` | `session.time_updated` within last 10s OR new `event` row in last 10s |
| `idle` | no event/message activity in last 30s, `time_archived` is null |
| `waiting` | TBD — needs capture from a real opencode run that hits a confirmation prompt or tool-use approval |
| `dead` | `time_archived IS NOT NULL` |

## Injection strategy

Same as Claude Code: `pbpaste` + AppleScript `do script` against Terminal.app (or iTerm2 via emulator-aware dispatch from the existing TODO item). opencode is a terminal-resident TUI, so the existing humOS injection works without modification.

For the launch demo: signal() broadcasts a single message; both the Claude Code window and the opencode window receive the paste and the user sees both agents respond.

## Spike findings — confirmed

1. opencode 1.14.30 installs cleanly via Homebrew.
2. State lives at `~/.local/share/opencode/opencode.db` with a clean drizzle schema.
3. Session rows are written even when the model call fails, meaning the adapter can begin matching cwd → session immediately.
4. `event` table exists and is the right surface for status detection but needs a successful run to capture concrete event types.
5. HTTP API in default mode is too narrow to use for v0.6.0. sqlite is the path.
6. Injection strategy from ClaudeParser maps directly. No new injection work.

## Spike findings — open

1. Concrete `event.type` vocabulary (need a successful opencode session to enumerate).
2. Whether `summary_diffs`/`summary_files` on session can drive a richer humOS card (file-changes preview).
3. Whether opencode auto-opens a window or requires manual TUI launch — affects how humOS surfaces the "Send" / "Focus" actions.

## Next concrete actions

| # | Action | Effort | Owner |
|---|--------|--------|-------|
| 1 | Authenticate opencode locally (`opencode providers login`) and run a real session to capture event types | S | Bolu |
| 2 | Open the db read-only from a Rust scratchpad, query session list by cwd | S | spike code |
| 3 | Add `OpenCodeReader` skeleton next to `ClaudeParser` in the workspace, wire to existing parser registry | M | v0.6.0 |
| 4 | Demo recording: Claude Code + opencode side-by-side, signal() broadcasts to both | S | launch |
| 5 | Update TODOS.md April 19 lock-in to substitute Codex → opencode as v0.6.0 first adapter (Codex moves to v0.7) | S | Bolu approval |
| 6 | LP + README copy: "Coordination primitives for any agent CLI on your Mac" — add opencode to the visible agent list | S | launch |
