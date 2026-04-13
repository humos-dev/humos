# humOS — Unix primitives for AI agent coordination

A native macOS app that gives you pipe, signal, and join for your running Claude CLI sessions. Route output between sessions automatically. Broadcast a constraint to all of them at once. Stop being the message bus.

Built for developers who run 3 to 20 parallel Claude Code sessions and are tired of tab-switching to relay context between them. Conductor spawns its own sandboxed sessions. opcode reads JSONL files. claude-control shows a dashboard. humOS operates on the real sessions you already have open, and gives you primitives to coordinate them.

The 10x insight: Unix gave developers fork, pipe, signal, and join to coordinate processes. Nothing equivalent exists for AI agents on your local machine. That's the layer humOS is building.

---

## Demo

![pipe() in action](docs/humos-pipe-demo.gif)

Session A finishes a schema. pipe() fires. Session B picks it up and writes tests. No human relay.
<!-- TODO: record -->

![signal() broadcast](docs/humos-signal-demo.gif)

One message. Every session receives it. Two-second undo in case you didn't mean it.
<!-- TODO: record -->

---

## What it does

- **`pipe()`.** Route output from session A to session B automatically. When A goes idle or writes a file matching a glob, a message drops into B's terminal. No human relay. Rules persist in `~/.humOS/pipe-rules.json` and survive restarts.
- **`signal()`.** Broadcast a single message to every active session at once. "Abort." "New constraint: don't touch auth.ts." "Pivot, here's the new direction." One click, all sessions receive it. 2-second undo window in case you typed something you shouldn't.
- **Session dashboard.** Real-time view of every Claude CLI session on your machine, with project name, working directory, status (running, waiting, idle), tool call count, and last output line. Sessions update live via file watcher on `~/.claude/projects`.
- **Per-card actions.** Focus brings the matching Terminal window to front. Send injects a message into one session. Summarize reads the JSONL, calls `claude -p`, and returns a two-sentence summary as a card overlay.

---

## The product is the primitives

The dashboard is what you see when you open humOS. It's useful on its own — live session status, one-click focus, instant summaries. But it's not the product. The product is what happens when you stop being the message bus between your sessions. pipe() fires and session B starts working without you touching a key. signal() redirects every agent at once. join() (coming) aggregates results when they're all done. The dashboard is the inspector. The primitives are the OS.

---

## Why this exists

You're running four Claude sessions. One is writing a schema. One is writing tests against it. One is refactoring the API. One is watching for regressions. The schema session finishes. Now you tab over, copy the file path, tab to the test session, paste it, hit enter. Then tab back. Then tab forward. You are the message bus. That's the problem humOS solves. The first time pipe() fires and a message lands in session B without you touching a key, the thing clicks.

---

## Install

### Option 1: Homebrew (recommended)

```bash
brew tap humos-dev/humos && brew install --cask humos
```

### Option 2: Download ZIP

1. Download `humOS_0.4.4_aarch64.zip` from [GitHub Releases](https://github.com/humos-dev/humos/releases/latest)
2. Unzip and drag **humOS.app** to Applications
3. Run: `xattr -cr /Applications/humOS.app`

**Requirements:**
- macOS 13 or later (Apple Silicon)
- Terminal.app or iTerm2
- Claude CLI installed and actively in use

---

## Quickstart

1. Launch humOS. Your Claude sessions appear automatically, sorted running → waiting → idle.
2. Click **Pipes**, add a rule (session A → session B, trigger: `OnIdle`), hit **Add**.
3. Do work in session A. When it goes idle, watch your pipe message land in session B's terminal.

That's the primitive. Everything else is variations on it.

---

## Status and roadmap

**Shipped (v0.4.4):**
- Session dashboard with live file watching
- `pipe()` with `OnIdle` and `OnFileWrite` triggers, persistence, and pipe-fired animations
- `signal()` broadcast with undo window, partial-failure reporting, and per-card flash states
- Focus / Send / Summarize per-card actions

**Next up:**
- `join()` — wait for multiple sessions to complete, then aggregate their outputs
- Orchestrator session — a Claude session that coordinates other sessions autonomously
- Agent agnosticism — Cursor, Aider, Codex CLI, and custom agents via `~/.humOS/sessions/<agent>/<id>.jsonl`
- iTerm2 support

Full backlog and primitive specs live in [`TODOS.md`](TODOS.md).

---

## Why now

Claude Code adoption is accelerating and multi-session workflows are becoming the default way power users work. The window to define what "coordination layer for local AI agents" means is open right now and it's maybe six months wide before someone else claims it. If you're already running four Claude sessions in parallel and acting as the message bus between them, you're already the target user.

---

## Development

```bash
git clone https://github.com/humos-dev/humos
cd humos
npm install
PATH="$HOME/.cargo/bin:$PATH" npm run tauri dev
```

Requires Rust (via [rustup](https://rustup.rs)) and Node.js.

---

## License

MIT — see LICENSE.

---

## Credits

Built by [@BoluOgunbiyi](https://github.com/BoluOgunbiyi). Inspired by Unix process primitives and too many open Claude tabs.
