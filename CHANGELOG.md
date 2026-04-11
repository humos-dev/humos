# Changelog

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
