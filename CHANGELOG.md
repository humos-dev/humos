# Changelog

## [Unreleased], Plan 2 Phase A

### Added
- **`humos-daemon` crate.** New standalone binary that owns the tantivy session index at `~/.humOS/index/` and serves it over a Unix socket at `~/.humOS/daemon.sock`. Ships as Phase A (PR 1 of 4) of Plan 2: the coordination runtime foundation. Phase B adds the MCP stdio server, Phase C migrates the app to consume the daemon and lights up the Project Brain ribbon, Phase D adds distribution.
- **Tantivy-backed keyword index.** Full-text search over session content (last_output + tools + project). Schema versioning with auto-rebuild on mismatch. Snippet pre-truncated at 120 chars by the backend. Reader reloads after every commit so writes are visible immediately.
- **Regex secret redaction.** Scrubs common credential patterns (Anthropic, OpenAI, AWS, GitHub, Slack, bearer tokens, private key blocks) before content enters the index. `HUMOS_INDEX_REDACT=off` disables for debugging. Defense-in-depth, not full DLP, raw JSONL is still on disk.
- **Newline-delimited JSON IPC.** Protocol over Unix socket supports `ping`, `health`, `search`, `related_context`, `bulk_related_contexts`, `stats`. Every error carries `{problem, cause, fix, docs_url}` so downstream clients can surface actionable messages.
- **`humos-daemon doctor` subcommand.** 7 preflight checks with fix guidance: config parse, humos home writable, index directory, schema version, socket path, Claude sessions discovery, another-daemon probe.
- **`~/.humOS/config.toml`.** Optional user config with `exclude_cwds`, `exclude_patterns`, `disable_project_brain`, `scan_days`. Missing file uses defaults.
- **Cargo workspace.** `src-tauri` and `humos-daemon` share a workspace root. `providers` module on `humos_lib` is now public so the daemon can reuse ClaudeProvider for session discovery.

### Not in this PR (Plan 2 later phases)
- `humos-mcp` stdio server exposing the IPC protocol as MCP tools (Phase B).
- humos-app consuming the daemon and the Project Brain ambient ribbon UI (Phase C).
- Homebrew tap, GitHub release binaries, launchd plist for daemon auto-start (Phase D).

## [0.4.5] - 2026-04-15

### Changed
- **README and landing page repositioned around primitives.** Hero now leads with "pipe, signal, and (soon) join for your running Claude CLI sessions. Stop being the message bus." The dashboard is the inspector. The primitives are the OS. Copy consistent across README and `docs/index.html`.
- **Removed speculative framing from README.** "Why now" section trimmed from competitive-window language to a user-facing observation. "10x insight" pitch-deck phrasing removed from the intro.

### Added
- **Demo GIFs** (`docs/humos-demo.gif`, `humos-pipe-demo.gif`, `humos-signal-demo.gif`), animated captures of the dashboard, pipe rules panel, and signal broadcast input. Referenced from README.

### Removed
- `STRATEGY.md` removed from tracking. Strategic planning belongs in private notes, not a public repo.

## [0.4.4] - 2026-04-12

### Changed
- **Distribution: DMG to ZIP.** macOS 26 blocks unsigned DMGs entirely. humOS now distributes as a ZIP archive (`humOS_0.4.4_aarch64.zip`). Install: unzip, drag to Applications, run `xattr -cr /Applications/humOS.app`. Build script is now `scripts/build-release.sh` (replaces `scripts/build-dmg.sh`).

## [0.3.7] - 2026-04-12

### Fixed
- **Send button broken after Claude CLI restart**: `inject_message` looked up the session's cwd by `session_id` in the live session map, but session IDs are JSONL filenames that change on every Claude CLI restart, so Send failed with `"Session … not found or has no cwd"` whenever the user restarted a CLI session. The frontend now passes `cwd` as a fallback; the backend resolves in order (session map → explicit cwd → error). Mirrors the v0.3.2 CWD fallback that fixed the same class of bug in `pipe()`.
- **Send input also gains server-side message validation**: the same empty/length/control-char sanitization that signal_sessions enforces now applies to the single-session Send path. Newline-in-message no longer fragments the Claude prompt mid-draft.
- **Signal command bar position**: reverted from absolute overlay to in-flow positioning. The bar now sits between the header and the card grid, pushing cards down 40px when open. This fixes the "input is hiding the first row of cards" bug introduced in v0.3.6. The UX audit flagged "layout shift jarring," but in practice the instant reflow is cleaner than the overlay hiding card content.

### Added (distribution scaffolding, not yet live)
- `.github/workflows/release.yml` — `tauri-action` release workflow, triggered by `v*.*.*` tags, builds + signs + notarizes on `macos-14`, drafts a GitHub Release with the .zip attached. Requires 6 Apple secrets to be added to repo settings before the first release.
- `docs/RELEASE.md` — release runbook covering prerequisites, the tag-and-push happy path, and three most common failure modes.
- `README.md` — full rewrite. Positioning pivoted from "session monitor with a v2.0 north star" to "shipped Unix primitives for AI agent coordination." Competitor comparison, install paths, quickstart, and the 10x line now all in the hero.
- `docs/index.html` — self-contained static landing page for humos.dev. Dark-themed HTML+CSS, zero JS, responsive at 640/1024 breakpoints.
- `homebrew/Casks/humos.rb` — Homebrew cask formula targeting Apple Silicon `.zip` from GitHub Releases, with zap cleanup and livecheck.
- `homebrew/README.md` — tap publishing runbook for the separate `homebrew-humos` repo Bolu will create before first release.
- `LICENSE` — MIT, 2026.

## [0.3.6] - 2026-04-11

### Fixed — signal() polish pass (from UX audit)
- **Layout shift on open**: `.signal-command-bar` is now `position: absolute; top: 73px; z-index: 50` with a box-shadow instead of sitting in the document flow. Opening the bar no longer pushes the session grid down 40px. Computer vision verified: cards stay put when the bar opens.
- **Countdown bar visibility**: `.signal-command-bar__toast::after` raised from 1px/0.6 opacity → 2px/0.8 opacity so the 2s undo countdown is actually noticeable.
- **Undo cancel hit target**: `.signal-command-bar__cancel` gained 4x8 padding + hover background so it's clickable as a button, not a thin underlined word.
- **Counter threshold + gradient**: the char counter now appears at 350+ chars in the subtle `--text-2` color and flips to red (`--warn` modifier) at 460+. Previously it only appeared at 409+ in red — a cliff with no warmup.
- **Screen-reader live region**: added a visually-hidden `role="status" aria-live="polite"` span inside the command bar that announces "Queued for N sessions, undo available for two seconds" on pending and error text on failure. The error banner itself also gained `role="alert"`.

## [0.3.5] - 2026-04-11

### Fixed — signal() QA pass
- **UTF-8 preview panic**: `signal_sessions` log preview previously sliced bytes (`&message[..message.len().min(60)]`) which panics on multi-byte UTF-8 char boundaries (emoji, non-ASCII). Now uses `message.chars().take(60).collect()`.
- **Server-side message validation**: `signal_sessions` now rejects empty/whitespace-only messages, enforces `SIGNAL_MAX_CHARS` (512) server-side in characters (not bytes), and replaces control characters (newlines, tabs) with spaces so broadcasts can't fragment the Claude CLI prompt mid-draft.
- **Empty-targets error**: `signal_sessions` now returns `Err("No active sessions.")` instead of silently emitting an empty broadcast when the non-idle session set has gone empty between button click and command fire.
- **Tokio worker blocking**: the inject loop (N × ~400ms AppleScript calls) now runs inside `tokio::task::spawn_blocking` so it can't tie up a tokio async worker for the full broadcast duration.
- **Signal flash/fail timeout leaks**: `setSignalFlashIds` and `setSignalFailIds` `setTimeout`s now live in `signalFlashTimeoutRef` / `signalFailTimeoutRef` and are cleared in the unmount cleanup effect (mirrors the v0.3.4 `animatePipeLine` pattern). Rapid re-signals now cancel the prior in-flight clear-timeout so a new flash set can't be clobbered mid-window.
- **Double-submit race**: `handleSignalSubmit` now hard-clears any stacked `signalUndoRef` timeout before starting a new one (stops two pending undo windows from firing back-to-back).
- **Empty `results` false-positive**: frontend now handles `results.length === 0` explicitly with an error banner instead of falling through the `allFailed` branch.
- **Partial-failure surfacing**: when some sessions fail, the failed project names are now listed in the error banner (up to 5) and the activity log (up to 3). Previously `fail_ids` round-tripped to the UI but were silently discarded.
- **Escape key decoupling**: Escape no longer simultaneously closes the Pipes drawer AND cancels a pending signal. The handler now dispatches to whichever modal is open, with Signal taking priority.
- **Pipes/Signal mutual exclusion**: opening the Signal command bar now closes the Pipes drawer (and vice versa). Previously both could be open simultaneously with stacked z-index and weird focus behavior.
- **1-session plural bug**: command bar placeholder was hardcoded `"all active sessions"` regardless of count. Now reads `Broadcast to ${N} session${s} — Enter to send, Esc to cancel`. Tooltip wording also clarified to `"Needs a running or waiting session"` when disabled.
- **Accessibility**: input gained `aria-label="Signal broadcast message"`; disabled Signal button gained `aria-label` + grayscale filter so screen readers and sighted keyboard users both get an unambiguous state.
- **Focus on re-open**: added `useEffect` on `signalOpen` that calls `signalInputRef.current?.focus()` — `autoFocus` only fires once on mount, so toggling the bar open a second time left the caret unfocused.
- **Log format**: activity log entry switched from `⌁ signal → N sessions: [preview]` to `⌁ N/M · [preview]` and suppresses the entry entirely when `success_count === 0` (a failure-only entry is emitted from `handleSignalSubmit` instead).

## [0.3.4] - 2026-04-11

### Fixed
- **pipe double-injection**: `PipeManager` now tracks `last_fired` per rule id and debounces fires within a 5s window. Previously, the file watcher and periodic rescan could both observe the same running→idle transition and each dispatch `inject_message`, causing duplicate messages in the target terminal.
- **glob recompilation**: compiled `Pattern`s are cached in `PipeManager.glob_cache` keyed by pattern string. The hot `evaluate` loop no longer recompiles the same glob on every tick.
- **animation timeout leak**: `animatePipeLine` now captures the nested `setTimeout` IDs and clears them in the returned cleanup, so unmounting the canvas mid-animation can't mutate a torn-down DOM node.
- **signal undo unmount race**: added a cleanup `useEffect` that clears `signalUndoRef` on App unmount, preventing a late `invoke("signal_sessions")` firing against a dead component.
- **PipeRule type drift (frontend)**: `App.tsx` `PipeRule` interface now includes the `trigger` field so it matches `PipeConfig`'s version exactly (fixes TS2719 nominal-type error).

## [0.3.3] - 2026-04-11

### Fixed
- **pipe display fix**: `PipeConfig` no longer manages its own `rules` state — rules are lifted to `App.tsx` and passed as props. Eliminated async setState race that caused persisted rules to silently disappear from the Pipes drawer on open.
- **pipe drawer load timing**: `pipeOpen` useEffect now calls `loadPipeRules()` unconditionally (open and close), ensuring the drawer always shows fresh data from backend.
- **debug logging removed**: `eprintln!` debug statements removed from `pipe_rules_path`, `load_pipe_rules`, and `list_pipe_rules`. Load errors now routed through `log::warn!`/`log::error!`.
- **load error resilience**: `load_pipe_rules` gracefully handles missing file (no error) and malformed JSON (logged error) without panic.

## [0.3.2] - 2026-04-11

### Fixed
- **pipe CWD fallback**: `PipeRule` now stores `from_cwd` and `to_cwd` at creation time. `evaluate` falls back to CWD matching when session IDs change (IDs are JSONL filenames — they change on every Claude CLI restart). Pipes now survive session restarts without the user needing to re-create rules.
- **pipe snapshot stability**: Snapshots now keyed by CWD (stable) instead of session ID (unstable), so edge detection (running→idle, last_output change) is preserved across restarts.
- **pipe periodic rescan gap**: `start_periodic_rescan` now calls `evaluate_pipes` after each rescan batch. Previously, session state updated but pipe rules were never evaluated in the rescan path — OnIdle transitions that the file watcher missed were silently dropped.
- **pipe rule persistence**: Rules now saved to `~/.humOS/pipe-rules.json` on add/remove and loaded on startup. Rules no longer lost when the app restarts.
- **`add_pipe_rule` command**: Now accepts optional `from_cwd`/`to_cwd` parameters; resolves them from the live session map when not provided.
- **Frontend `PipeRule` interface**: Added `from_cwd` and `to_cwd` fields to match updated backend struct.

## [0.3.1] - 2026-04-11

### Fixed
- **pipe `OnFileWrite`**: no longer fires on assistant text that mentions a filename (e.g. "I updated schema.json"). Now guards with `starts_with("Running:")` so only actual tool invocations trigger the rule
- **pipe `pipe-fired` event**: now emitted AFTER `inject_message` completes, with `success: bool` and `error: Option<String>` fields. UI no longer shows the pipe animation for injections that silently failed
- **signal button tooltip**: "No running sessions" → "No active sessions" (waiting sessions also enable Signal)
- **signal placeholder**: "Broadcast to all running sessions" → "Broadcast to all active sessions"
- **signal undo toast**: "Sending" → "Queued", "Cancel" → "Undo" (more accurate — action hasn't fired yet)
- **signalUndoRef**: nulled immediately when the 2-second window fires, preventing stale ref on subsequent signals
- **signal command bar**: placeholder contrast raised (#444 → #666), left-border accent added, error state gets red background tint, countdown animation on undo toast

## [0.3.0] - 2026-04-11

### Added
- **signal()**: broadcast a message to all running+waiting sessions simultaneously with one click
  - `signal_sessions` Tauri command (Rust): iterates all non-idle sessions, calls `inject_message` for each, emits `signal-fired` event with per-session success/fail split
  - `⌁ Signal` button in header — disabled (greyed, tooltip) when 0 non-idle sessions
  - Signal command bar: 40px overlay below header, auto-focused input, 512-char limit with counter at 80% capacity
  - 2-second undo window: toast shows "Sending to N sessions — Cancel" before inject fires
  - All-fail error state: inline red error in command bar, input stays open
  - Session card flash animations: green ripple on successful delivery, red glow on failure (distinct timing from pipe animation)
  - Activity log entry: `⌁ signal → N sessions: [preview]` with fail count if partial

## [0.2.1] - 2026-04-11

### Added
- `PLAN-signal.md`: fully reviewed plan for `signal()` (Primitive 2). Passed 4-phase autoplan review (CEO, Design, Eng, DX). 19 decisions logged.
- TODOS.md: 6 new deferred items — signal() v2 selective broadcast, signal vocabulary, programmatic API, file-based signaling, parallel injection (N>15), and humOS runtime model spec

## [0.2.0] - 2026-04-11

### Added
- **Pipe system**: connect any two sessions — when session A goes idle or writes a matching file, inject a message into session B's terminal automatically
- Pipe rules persist in app state and survive across sessions; add/remove via the Pipes drawer
- Canvas animation when a pipe fires: dashed green line traces from source to target card, target flashes with border highlight
- Activity log bar at bottom of screen: last 5 pipe events with timestamps, persisted across restarts via localStorage
- `start_periodic_rescan` background thread: rescans recently-modified files every 5 seconds for sessions the file watcher misses (handles large JSONL files, 60s lookback window)
- **Multi-agent platform vision**: documented humOS Agent SDK spec in TODOS.md — designed to support Claude Code, Cursor, Copilot, Aider, Codex CLI, Cline, Devin, and custom agents via `~/.humOS/sessions/<agent>/<id>.jsonl`

### Changed
- Renamed **HumOS** → **humOS** everywhere (dock, title bar, product name, Cargo lib, README)
- Poll interval reduced from 30s to 5s for faster session freshness
- `inject_message` now uses pbpaste approach — writes message to clipboard, then runs `pbpaste` in the matching Terminal tab. Eliminates shell injection risk of embedding content inside `do script "..."` AppleScript
- `compute_status` restores mtime gate — sessions whose last role was "assistant" but haven't been modified in >5 minutes show as `idle` instead of `running` forever
- Icon regenerated via `tauri icon` pipeline: pure black background, three `#5fffb8` waveform bars, no border frame
- Session cards show pipe source/target indicators (subtle 5px green dot)
- `logSeq` moved from module-level mutable to `useRef` inside App component

### Fixed
- Pipe `OnIdle` trigger no longer false-fires on first evaluation tick for already-idle sessions
- Pipe `OnFileWrite` trigger no longer false-fires on startup (treats first tick as no-change)
- `snapshots` map in PipeManager now prunes stale entries when sessions are removed, preventing unbounded memory growth
- `last_segment` in AppleScript tab matching now properly escaped through `escape_applescript()`
- `sessions.lock().unwrap()` replaced with `unwrap_or_else(|e| e.into_inner())` across all call sites — recovers gracefully if any thread panics while holding the mutex
- `animatePipeLine` requestAnimationFrame loop now correctly cancelled on component unmount

## [0.1.0] - 2026-04-11

### Added
- Native macOS Tauri v2 app monitoring all active Claude CLI sessions in real-time
- File watcher on `~/.claude/projects` with 200ms debounce — sessions update live
- Session cards: project name, cwd, status dot (running/waiting/idle), tool count, last output
- Status detection: `running` when Claude is actively calling tools, `waiting` when expecting user input, `idle` otherwise
- Sort order: running → waiting → idle, then by most recently modified
- Date/time tags: Today, Yesterday, Xd ago (≤6 days), or MMM D format
- **Focus** button: AppleScript brings matching Terminal window/tab to front (matches auto-generated tab name and cwd)
- **Send** button: inject message into terminal via clipboard + keystroke simulation
- **Summarize** button: reads session JSONL, calls `claude -p` with `--no-session-persistence`, returns 2-sentence plain English summary
- Summary renders as absolute overlay on card — does not shift grid layout
- Animated loading dots in Summarize button while generating
- JSONL parser correctly reads real Claude CLI session format (cwd/sessionId on every line, not a special init event)
