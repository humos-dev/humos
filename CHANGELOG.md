# Changelog

## [0.6.1] - 2026-05-01

### Fixed
- **Activity log no longer drops or duplicates entries on app restart.** The log entry id counter reset to 0 on every app launch, but the log itself was persisted to localStorage with ids 0..N from the prior session. New entries got ids starting at 0 again, colliding with restored ones. React's reconciliation could silently drop or duplicate entries during render. Fixed by seeding the counter from the highest persisted id, plus a composite React key (`${id}-${ts}`) for defense in depth.
- **Test-mode banner link points at `/releases/latest` instead of a constructed `/tag/v<fake>` URL.** When a developer tests the banner UI by setting `localStorage.humos-test-update-banner` to a fake version, the link now resolves to a real GitHub releases page rather than a 404. Production code path unchanged.

### Internal
- New `scripts/find-stale-installs.sh` detects orphan `humOS.app` copies in `~/Downloads`, `~/Desktop`, `~/Documents`, `$HOME`, `/tmp`, and mounted volumes. Optional `--delete` mode removes them after a confirmation prompt. Documented in `RELEASING.md`.
- `scripts/release.sh` now runs `scripts/preflight.sh` (cargo test, tsc, hook installed, version sync) before any mutation, and `scripts/find-stale-installs.sh` is available for post-install cleanup. Pre-commit hook now also enforces version-source sync when any of the three version files are staged.

## [0.6.0] - 2026-05-01

### Added
- **Cross-vendor coordination via the Provider trait.** humOS now coordinates sessions across multiple agent CLIs at once. The first non-Claude adapter ships in this release: opencode (sst/opencode). Sessions appear in the dashboard with a `provider: opencode` badge alongside Claude Code sessions. Adding a third agent CLI in the future is a new file plus a registry entry, no changes to dispatch.
- **OpenCodeProvider.** Reads opencode's sqlite database at `~/.local/share/opencode/opencode.db` (XDG path, honors `XDG_DATA_HOME`) read-only. Maps session rows to humOS `SessionState` with status derived from `time_updated` recency. All sqlite failure modes (open, prepare, query) log warnings so a future opencode upgrade that renames a column surfaces in logs instead of silently returning empty results.
- **`signal()` fans across every registered provider.** `signal_sessions` now calls `ProviderRegistry::broadcast`, which iterates each registered provider and aggregates per-provider tab counts. A single signal hits Claude tabs and opencode tabs in one call.
- **`broadcast_to_terminal_tabs_running(process_name, message)`.** Generic AppleScript helper extracted from the previous Claude-only `broadcast_to_all_claude_tabs`. Empty `process_name` is rejected to prevent broadcasting to every Terminal tab on the machine.
- **OPENCODE badge in the session card.** Orange (`#fb923c`) accent that is visually distinct from Claude's green and Codex's purple.
- **Test-mode update banner.** Setting `localStorage.humos-test-update-banner` to a version string forces the update banner to render against that fake version, bypassing the fetch and the dismiss-key check. Lets you verify the banner UI in dev or prod without shipping a real release. Clear with `localStorage.removeItem("humos-test-update-banner")`.
- **`scripts/release.sh`.** One command that does the entire release: bumps `tauri.conf.json`, syncs `Cargo.toml` and `package.json`, builds the .app and ZIP, regenerates `docs/version.json`, commits, tags, pushes, and creates the GitHub release with notes pulled from the matching CHANGELOG section. Has `--dry-run` and safety checks for branch, working tree, gh CLI presence, and CHANGELOG section presence.
- **`scripts/sync-versions.sh`.** Reads `tauri.conf.json` as canonical and propagates the version to `Cargo.toml [package]` and `package.json`. `--check` mode exits 1 on drift.
- **`scripts/hooks/pre-commit` and `scripts/install-hooks.sh`.** Tracked pre-commit hook that blocks em dashes anywhere, AI-slop vocabulary in `.md` and `.html` files, and version-source drift when any of the three version files are staged. Run `./scripts/install-hooks.sh` once after clone.
- **`PLAN-opencode-adapter.md`** in repo root. Spike report documenting the integration path decision (sqlite poller vs. HTTP API vs. ACP) and the schema mapping from opencode's session table to humOS `SessionState`.

### Changed
- **Landing page and README copy is agent-agnostic.** `Claude Code sessions` became `agent CLI sessions` in headlines, meta tags, and install requirements. Hero subline corrected so `pipe and signal ship today, join() next` no longer overstates `join()` as present-tense. Install requirements list both Claude Code and opencode with links. The "How it works" step that named only `~/.claude/projects/` now lists both data sources to match the FAQ.
- **Update check errors are visible.** `useVersionCheck.ts` replaced the silent `catch {}` with `console.warn("humOS update check failed:", err)`. Network errors, the 3s timeout, and JSON parse failures all surface in the devtools console.

### Fixed
- **Active session no longer masked by older sibling JSONL.** When Claude Code dispatches subagents via the Agent tool, each subagent writes its own JSONL file stamped with the parent session's id. `scan_sessions_into` was using last-write-wins via `HashMap::insert`, which routinely picked an older agent file over the active conversation, leaving the active session stuck displaying as idle. Now uses `merge_sessions_by_newest` keyed on `modified_at`. The most recently modified file per session id always wins regardless of insertion order.
- **opencode state path on macOS.** `dirs::data_dir()` returns `~/Library/Application Support` on macOS but opencode writes to `~/.local/share/opencode/` on every platform. The previous code returned `None` silently and humOS scanned zero opencode sessions. Now honors `XDG_DATA_HOME` if set, falls back to `~/.local/share/opencode/`.

### Internal
- Bumped `Cargo.toml` and `package.json` to 0.5.6 and added a sync script. `tauri.conf.json` was the canonical version source and had been at 0.5.6 for several releases; the other two files had drifted to 0.4.4.
- `humOS.txt` and other Claude Code session exports gitignored. The previously-untracked `humOS.txt` was relocated to `~/.gstack/projects/humos/`.
- `RELEASING.md` documents the release flow end to end including how to test the banner before each release and how to roll back.

## [0.5.6] — 2026-04-25

### Fixed
- **Update banner "See what's new" link no longer 404s.** The link now uses the `url` field from `version.json` (controlled by the build script) instead of constructing a URL from the version number. This ensures the link always points to a real published release.

## [0.5.5] — 2026-04-25

### Added
- **In-app update notifications.** humOS now checks for new versions on startup. When a newer version is available, a coord blue strip appears between the header and the session grid: "↑ humOS X.Y.Z available · See what's new ↗ · ×". Dismissed per-version — each new release re-triggers the banner. Polling uses `humos.dev/version.json` (served by Vercel, no rate limits). Silent on network errors and offline.
- **`docs/version.json`** — new static file served by Vercel at `humos.dev/version.json`. Updated automatically by `build-release.sh` on every release.

### Changed
- **`build-release.sh`** now auto-writes `docs/version.json` after every build so the update endpoint is always in sync with the shipped version.

## [0.5.4] — 2026-04-25

### Fixed
- **Send button now available in list view.** List view Actions column has full parity with grid view: Focus, Send, and Summarize. Clicking Send in list view shows an inline input row below the session. Idle sessions show "Ended" in the Send slot, same behaviour as grid view.

## [0.5.3] — 2026-04-25

### Fixed
- **Ghost pipe edges cleared when rule is deleted.** Removing a pipe from the Pipes drawer now immediately clears its canvas line. Previously the edge remained on screen until the next resize or pipe fire. Deleting all rules now clears the canvas entirely.
- **List view has full feature parity with grid view.** Focus and Summarize buttons are now available on every list row in an Actions column. Summary output appears as an inline detail row below the session row when Summarize is clicked.

## [0.5.2] — 2026-04-25

### Fixed
- **Dead session resume command shows full session ID.** The callout previously displayed a truncated 8-character ID. It now shows the complete `claude --resume <full-id>` command. Copy button pastes the full executable command directly — no extra steps needed.

## [0.5.1] — 2026-04-25

### Added
- **Persistent pipe edges.** Pipe connections are now always visible as blue lines between session cards — not just during the 500ms fire animation. Active connections (at least one session running) show as solid lines with arrowheads. Dormant connections (both sessions idle) show as dashed grey lines. Off-screen cards skip drawing cleanly.
- **Pipe history footer.** Each session card shows the last pipe direction and relative time ("→ humos · 2 min ago" or "← medwrite · just now"). Updates live as pipes fire.
- **Dead Session Indicator.** Idle sessions now show an "Ended" button instead of "Send." Clicking it reveals an inline callout with the resume command (`claude --resume <id>`) and a one-click copy button. Works in both grid and list view.
- **Grid / List view toggle.** New segmented control in the header lets you switch between the card grid and a dense row-based list view. List view shows session name, status, last output, pipe connection, and timestamp in a compact table layout. Choice persists to localStorage.
- **Design system (DESIGN.md).** Full design system committed: JetBrains Mono everywhere (loaded via `@fontsource/jetbrains-mono`), `--coord` (#3b82f6) for all coordination elements, `--amber` (#f59e0b) for waiting sessions, `--grid-line` (#0e0e0e) for the coordinate-space grid background.

### Changed
- **Semantic color split.** Green (`--signal`) now means session health only. Blue (`--coord`) means coordination — pipe edges, pipe dots, signal bar accent, pipe fire animation, pipe badge in header. The two were previously conflated.
- **Card radius and padding.** Session cards are now 5px radius (was 8px) and 12px padding (was 16px). Monitoring-tool density, not consumer-app cards.
- **Coordinate grid background.** The main session area now renders a subtle 24px grid in the background, reinforcing the "sessions as nodes on a surface" mental model.
- **Activity log.** Displays 8 entries (was 5) with a gentler opacity fade floor of 0.15.
- **BrainRibbon bleed margins** updated to match new 12px card padding.

## [0.5.0] — 2026-04-22

### Added (Plan 2 Phase C — Daemon IPC + Project Brain Ribbon)

### Added
- **`humos-client` crate.** Extracted `IpcClient` from `humos-mcp` into a shared workspace crate so any future consumer (app, CLI tools) can reuse the Unix socket client without duplicating code. 6 integration tests using in-process mock sockets cover ping, health, ENOENT, empty response, timeout, and JSON serialization.
- **Daemon IPC client in the Tauri app (`daemon_client.rs`).** Uses raw JSON strings over the Unix socket to avoid a circular dependency that would arise from importing `humos-daemon` types (which transitively import `humos_lib`, which is the app itself). Protocol matches `humos-daemon` exactly. Two public functions: `poll_health()` and `fetch_related_context(cwd)`.
- **Project Brain ribbon (v2, spec-conformant).** Ambient green strip at the top of every session card, always visible when related sessions exist. Matches the approved wireframe (`ribbon-wireframe.html` v2, post /design-review + /devex-review audit). Features: `▸_` glyph, click-to-expand session list with snippets + relative time, chevron affordance, dismiss button (in-memory Set, resets on restart), skeleton shimmer loading state, amber stale-index state, keyboard nav (Tab/Enter/Esc/ArrowUp/ArrowDown), dense-grid desaturation (>3 ribbons), provider badge demoted to neutral when ribbon present. Bulk fetch via `useRelatedContexts` hook (1 IPC call per unique cwd, not per card). Daemon-offline ribbon state deliberately suppressed until daemon ships as auto-start service.
- **Ribbon single-cwd suppression.** Ribbon is hidden when all visible sessions share the same working directory (no distinguishing information). Activates automatically when sessions span 2+ distinct project directories. Prevents every card from showing identical "N related sessions" noise.
- **Two new Tauri commands.** `check_daemon_health` (returns online/index_sessions/uptime) and `get_related_context(cwd)` (returns RibbonResult). Both use `spawn_blocking` to avoid blocking the async runtime on Unix socket I/O.
- **TODOS.md entries.** Adaptive Poll Interval (P2, S) and Daemon Version Handshake (P2, S) captured as Phase C follow-ons.

### Changed
- **Session poll replaces file watcher.** `notify` and `notify-debouncer-mini` removed from `src-tauri`. A simple `std::thread::spawn` loop sleeping 5s calls `scan_sessions_into` on each tick. Frontend's existing 5s `setInterval` on `get_sessions` provides the UI refresh.
- **`ProviderRegistry` removed from the app.** `focus_session`, `inject_message`, `signal_sessions`, and `dispatch_pipe_action` now call AppleScript helpers directly. `ClaudeProvider` is used directly for session scanning.
- **`codex.rs` provider deleted** (64 LOC). Unused since Codex CLI is not part of the Phase C scope.

### Architecture Notes
- **Why raw JSON in `daemon_client.rs`:** Importing `humos-daemon` types in `src-tauri` creates a cycle (`humos` → `humos-daemon` → `humos_lib` → `humos`). Raw JSON string IPC breaks the cycle at the cost of stringly-typed request construction. Protocol is simple (5 message types) and tested in `humos-client` integration tests.
- **Why app still owns session state:** The daemon protocol has no `ListSessions`. `SearchResult` carries `{id, cwd, project, snippet, score}` — no `status`, `tty`, or `tool_count`. The app's JSONL parser remains the source of truth for session status. Daemon is used for Health + RelatedContext only.

## [0.5.0 cont.] — Plan 2 Phase B

### Added
- **`humos-mcp` crate.** New standalone binary that speaks MCP (Model Context Protocol) JSON-RPC 2.0 over stdio and bridges any MCP-capable AI agent (Claude Code, Codex CLI, Cursor) to the humOS daemon. Ships as Phase B (PR 2 of 4) of Plan 2.
- **Four MCP tools exposed.** `search_sessions(query, limit)` for keyword search, `list_sessions(cwd?, limit)` for recent sessions (optionally filtered by cwd), `get_project_context(cwd, limit)` for Project Brain (recall past work in the same repo), `humos_health` for daemon status.
- **`humos-mcp doctor` subcommand.** 4 checks: daemon socket exists, daemon reachable via Ping, tools surface registered, health probe returns. Prints MCP client config snippets for Claude Code, Codex CLI, and Cursor.
- **Error translation.** Daemon IPC errors propagate to MCP clients as `isError: true` tool results with the problem / cause / fix / docs_url body so the calling agent can surface actionable messages.
- **README** with copy-paste JSON/TOML config for all three MCP clients.

## [0.5.0 cont.] — Plan 2 Phase A

### Added
- **`humos-daemon` crate.** New standalone binary that owns the tantivy session index at `~/.humOS/index/` and serves it over a Unix socket at `~/.humOS/daemon.sock`. Ships as Phase A (PR 1 of 4) of Plan 2: the coordination runtime foundation. Phase B adds the MCP stdio server, Phase C migrates the app to consume the daemon and lights up the Project Brain ribbon, Phase D adds distribution.
- **Tantivy-backed keyword index.** Full-text search over session content (last_output + tools + project). Schema versioning with auto-rebuild on mismatch. Snippet pre-truncated at 120 chars by the backend. Reader reloads after every commit so writes are visible immediately.
- **Regex secret redaction.** Scrubs common credential patterns (Anthropic, OpenAI, AWS, GitHub, Slack, bearer tokens, private key blocks) before content enters the index. `HUMOS_INDEX_REDACT=off` disables for debugging. Defense-in-depth, not full DLP, raw JSONL is still on disk.
- **Newline-delimited JSON IPC.** Protocol over Unix socket supports `ping`, `health`, `search`, `related_context`, `bulk_related_contexts`, `stats`. Every error carries `{problem, cause, fix, docs_url}` so downstream clients can surface actionable messages.
- **`humos-daemon doctor` subcommand.** 7 preflight checks with fix guidance: config parse, humos home writable, index directory, schema version, socket path, Claude sessions discovery, another-daemon probe.
- **`~/.humOS/config.toml`.** Optional user config with `exclude_cwds`, `exclude_patterns`, `disable_project_brain`, `scan_days`. Missing file uses defaults.
- **Cargo workspace.** `src-tauri` and `humos-daemon` share a workspace root. `providers` module on `humos_lib` is now public so the daemon can reuse ClaudeProvider for session discovery.

### Not in this PR (Plan 2 later phases)
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
