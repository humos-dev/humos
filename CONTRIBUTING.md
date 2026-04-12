# Contributing to humOS

Thanks for your interest in humOS. This guide covers everything you need to get started.

If you're looking for something to work on, check the [issue tracker](https://github.com/humos-dev/humos/issues). Issues tagged `good-first-issue` are scoped for new contributors.

---

## Prerequisites

- **macOS 13+** (humOS is macOS-only, uses AppleScript for terminal control)
- **Rust** via [rustup](https://rustup.rs)
- **Node.js 20+**
- **Claude CLI** installed and signed in (needed to test session detection)

---

## Development Setup

```bash
git clone https://github.com/humos-dev/humos.git
cd humos
npm install
PATH="$HOME/.cargo/bin:$PATH" npm run tauri dev
```

The app will open with hot reload. Rust changes trigger a full rebuild. TypeScript/React changes are instant via Vite.

---

## Running Tests

**Rust unit tests:**

```bash
cd src-tauri && cargo test --lib
```

**TypeScript type checking:**

```bash
npx tsc --noEmit
```

Run both before opening a PR. CI will catch it if you don't.

---

## Architecture Overview

humOS is a Tauri v2 app. The Rust backend (`src-tauri/src/`) handles session discovery, pipe rule execution, signal broadcasting, and AppleScript integration. The React frontend (`src/`) renders the session dashboard, pipe configuration UI, and signal controls. Communication between the two layers happens over Tauri's IPC bridge via `#[tauri::command]` functions invoked from TypeScript.

Key backend files: `lib.rs` (app setup and commands), `pipe.rs` (pipe rule engine), `parser.rs` (JSONL session parsing), `applescript.rs` (Terminal.app integration).

Key frontend files: `App.tsx` (root layout and session polling), `SessionCard.tsx` (per-session card), `PipeConfig.tsx` (pipe rule editor).

---

## Pull Request Guidelines

- **One fix per PR.** Don't bundle unrelated changes.
- **Describe what changed and why** in the PR body. If the "why" isn't obvious, explain the problem you're solving.
- **Include screenshots** for any UI changes. Before/after is ideal.
- **Update CHANGELOG.md** if your change is user-facing.
- **Keep commits clean.** Squash fixups before requesting review.

---

## Code Style

**General:**
- No em-dashes in comments or docs. Use commas or periods.
- No AI-generated filler vocabulary ("leverage", "utilize", "streamline", "robust").

**Rust:**
- Follow `cargo clippy` defaults. Fix all warnings before submitting.
- Use `cargo fmt` for formatting.

**TypeScript:**
- Follow the existing `tsc --strict` config. No `any` types unless absolutely necessary.
- Prefer named exports.

---

## Issue Labels

| Label | Description |
|-------|-------------|
| `bug` | Something is broken |
| `enhancement` | New feature or improvement |
| `good-first-issue` | Scoped for new contributors |
| `pipe` | Related to the pipe() primitive |
| `signal` | Related to the signal() primitive |
| `distribution` | Packaging, Homebrew, DMG, auto-update |

---

## Questions?

Open an issue or start a [discussion](https://github.com/humos-dev/humos/discussions). We'll get back to you.
