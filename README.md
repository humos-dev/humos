# humOS

A native macOS app that monitors all active Claude CLI sessions in real time.

The name comes from the idea of sessions humming in the background while you work on something else.

---

## What it is

humOS shows you every Claude CLI session running on your machine: what project it is in, what directory it is working in, whether it is actively calling tools, waiting for input, or idle, how many tools it has called, and what it last output.

You can bring any session's terminal window to front, inject a message into it, or get a two-sentence summary of what it has been doing.

---

## Why it exists

Claude CLI sessions run in terminal windows. When you have more than one going, you have no way to see their state without switching to each terminal and reading the output yourself. There is no coordination layer. There is no way to route output from one session to another, broadcast a message to all running sessions, or wait for a set of sessions to complete before acting on the results.

Unix solved this for processes decades ago with fork, pipe, signal, and join. Nothing equivalent exists for AI agents at the local machine level.

humOS starts as a session monitor. The v2.0 north star is to become that coordination layer: a set of primitives for orchestrating AI agents running locally, the same way Unix gave primitives for orchestrating processes.

---

## v0.1.0

- **Session cards** -- project name, working directory, status dot, tool call count, last output line
- **Status detection** -- running (actively calling tools), waiting (expects user input), idle (otherwise)
- **Auto-sort** -- running first, then waiting, then idle, then by most recently modified
- **File watcher** -- watches `~/.claude/projects` with 200ms debounce
- **Focus** -- AppleScript brings the matching Terminal window or tab to front
- **Send** -- injects a message into the terminal via clipboard and keystroke
- **Summarize** -- reads the session JSONL, calls `claude -p --no-session-persistence`, returns a two-sentence summary rendered as a card overlay

---

## v2.0 north star

- `pipe()` -- route output from session A as input to session B automatically
- `signal()` -- broadcast a message to all running sessions simultaneously
- `join()` -- wait for multiple sessions to complete, then aggregate their outputs
- **Orchestrator session** -- a Claude session that monitors and coordinates other sessions autonomously
- **Task compiler / DAG executor** -- describe a goal, humOS decomposes it into parallel sub-sessions and manages execution

---

## Requirements

- macOS
- [Rust](https://rustup.rs) via rustup
- Node.js
- Claude CLI installed and actively in use

---

## Dev setup

```bash
git clone https://github.com/BoluOgunbiyi/humos
cd humos
npm install
PATH="$HOME/.cargo/bin:$PATH" npm run tauri dev
```
