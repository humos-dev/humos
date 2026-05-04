## STRATEGIC LOCK-IN (2026-04-19): Cross-Vendor Pivot

**Decision:** humOS's next-ship priority is cross-vendor coordination, not more Claude-Code-only polish.

**Why now:** Anthropic shipped `--worktree`, desktop multi-session sidebar, and agent teams (swarms) in the last two weeks. Every coordination feature humOS has today, Anthropic is also shipping natively. The race is unwinnable on Claude-only turf. The one layer Anthropic cannot build: coordination *across* CLIs (Claude Code + Codex + Cursor + Cline). That is where humOS becomes defensible.

**v0.5.0 retro:** v0.5.0 shipped Daemon IPC + Project Brain ribbon, NOT the cross-vendor adapter originally scoped. v0.5.1–v0.5.6 (Apr 25) shipped polish on the Claude-only surface (persistent pipe edges, list view, design system, in-app updater). The cross-vendor scope below moves to v0.6.0.

**v0.7.0 scope (target: after demo video + opencode waiting status + daemon bundle):**

*(v0.6.0 shipped 2026-05-01: OpenCodeProvider, signal() cross-provider, agent-agnostic LP/README, release.sh automation. Items below are what did not ship from that scope, plus new additions.)*
- Add `OpenCodeProvider` next to `ClaudeProvider` in `src-tauri/src/providers/`. Open opencode's sqlite database (`~/.local/share/opencode/opencode.db`) read-only and map sessions to humOS `SessionState`. Spike confirmed integration path 2026-05-01. See `PLAN-opencode-adapter.md`.
- Per-CLI injection strategy. `pbpaste` + Terminal already works for any Terminal-resident TUI, so opencode rides the same path as Claude.
- **opencode `waiting` status:** Run a real opencode session through a confirmation prompt, query `SELECT DISTINCT type FROM event` in the sqlite db, map the waiting-state event type in `compute_status()` in `opencode.rs`. Currently only `running`/`idle`/`dead` are mapped. `waiting` is when signal() is most valuable and it is undetectable without this.
- New demo video: Claude session + opencode session side by side, signal() broadcasts one prompt to both. See standalone TODOS entry below for scope.
- LP + README rewrite. Strip Claude-Code-specific copy. New line: "Coordination primitives for any agent CLI on your Mac." Add opencode to the visible agent list. **Copy must differentiate shell pipe from session pipe:** Anthropic's own docs use "pipe" to mean `command | claude` (synchronous, single-turn, human-triggered). humOS pipe() is session-to-session, async, agent-triggered, no human in the loop. Without this distinction, developers who just read the Claude Code docs will think they already have what humOS offers. Suggested line: "Shell pipes connect commands. Session pipes connect agents."
- Homebrew cask + GitHub ZIP ship with new tagline.

**Why opencode (not Codex CLI) as the first non-Claude adapter:** opencode is open source, sqlite-backed (clean schema, not reverse-engineered), no API-key dependency for testers, and demos with the same cross-vendor weight. Codex CLI moves to v0.7.

**v0.7.0:** Codex CLI adapter (process-based detection, second cross-vendor proof point).
**v0.8.0+:** Cursor + Cline adapters (IDE-embedded, harder injection path).

**Launch sequencing (critical):**
- v0.6.0 shipped OpenCodeProvider on 2026-05-01. The cross-vendor proof condition is met.
- Gate for public tagging (@bcherny, @claudeai): demo video showing Claude + opencode side by side + opencode waiting status + LP copy differentiating shell pipe from session pipe. All three must land before the launch post.
- Post hook: "agent teams is great for intra-session. humOS is for inter-session and cross-vendor. curious how you think about the split."

**North star reframe:** humOS is not a Claude Code tool. humOS is the Switzerland of agent CLIs. Every line of copy, every demo, every reply reflects that by the time v0.5.0 ships.

**Derivative killed:** any feature that only makes Claude-Code-on-humOS marginally better is deprioritized below any feature that adds a new CLI. Polish is dead weight; breadth is the moat.

---

## Feature Ideas (Backlog)

### Cross-Vendor Demo Video
**What:** Screen recording showing a Claude Code session and an opencode session running side by side in the dashboard. A single signal() broadcast fires to both. Both agents respond. The video must show output quality, not just the mechanic.
**Why:** The cross-vendor proof shipped in v0.6.0 but has no evidence a new user can watch. Without the video, the LP claim ("coordinates any agent CLI") is text. With the video, it is a 30-second proof. Also answers Jonathan Atiene's "I need to also observe output quality" signal from DISCOVERY.md.
**Scope:** Two terminal windows open. Claude Code session in one (working on a real coding task). opencode session in the other. Hit signal() in humOS. Both sessions receive the message and respond. Under 60 seconds. Annotated GIF for the LP, full MP4 for the GitHub release notes.
**Prerequisite:** opencode waiting status must be mapped first so the demo does not broadcast to a session mid-response.
**Effort:** S | **Priority:** P1 | **Target:** v0.7.0

### Bundle Daemon in ZIP Install (Distribution Fix)
**What:** Include `humos-daemon` binary inside `humOS.app` as a bundled Tauri resource. Auto-spawn it on app startup if the daemon socket is not responding. Users get Project Brain ribbon on first install with no manual steps.
**Why:** Project Brain ribbon is the answer to the P0 adoption barrier ("I use one session because of memories"). It requires the daemon. The daemon is not in the ZIP. New users never see the ribbon. This is a distribution problem, not a product problem. Confirmed by DISCOVERY.md Finding 1.
**Phase 1 (v0.6.0): three file changes, no new dependencies:**
1. `src-tauri/tauri.conf.json`: add `"resources": ["../target/release/humos-daemon"]` to `bundle`. Tauri copies the binary into `Contents/Resources/` at build time.
2. `src-tauri/src/lib.rs`: add `try_auto_start_daemon(app_handle)` called from `.setup()`. Calls `poll_health()`. If offline, resolves resource path via `app.path().resource_dir()`, spawns `humos-daemon run` as a child process, sleeps 600ms for socket readiness.
3. `scripts/build-release.sh`: add guard: if `target/release/humos-daemon` missing, run `cargo build --release -p humos-daemon` before tauri build.
**Phase 2 (v0.6.1): LaunchAgent persistence:**
4. On first launch (detect via absence of `~/Library/LaunchAgents/dev.humos.daemon.plist`), write the plist pointing at the bundled binary path and call `launchctl load`. Daemon now survives app close. Enables "while you were away" pipe firing.
**Edge cases:** second spawn attempt fails to bind socket and exits cleanly (first instance keeps running). macOS quarantine cleared by existing `xattr -cr` in build script, which covers bundled resources.
**Effort:** M (Phase 1), S (Phase 2) | **Priority:** P0 | **Target:** v0.7.0 (Phase 1), v0.7.1 (Phase 2)

### Provenance Headers on Pipe Injections
**What:** Add `modified_at` timestamp and source session name to every `build_message()` payload. Format: `[session 'api' @ 2026-05-03T14:32:05Z] File matching '*.json' written by session 'api': ...`
**Why:** The receiving session has no freshness signal. It processes injected context without knowing when it was generated. A timestamp lets the model self-detect stale context ("this came in 3 minutes ago, is it still valid?"). Strengthens pipe() reliability as a coordination primitive without adding any new plumbing.
**How:** `build_message()` in `pipe.rs` already receives `source: &SessionState` which carries `modified_at`. Prefix the timestamp and session name to both trigger branches. Three-line change.
**Effort:** S | **Priority:** P1 | **Target:** v0.7.0

### Confirm Dialog for Bulk Signal Broadcast
**What:** When signal() targets more than 3 sessions, show an inline confirmation: "Broadcasting to N sessions. Continue?" with the session list visible before sending.
**Why:** Single-session signals are low-risk. Broadcasting to 5+ sessions with a typo or wrong target is expensive to recover from; each session may have already acted before the user notices the mistake. Calibrated friction at the right threshold, not everywhere.
**How:** In `App.tsx` signal send handler, count non-idle target sessions before firing. If count > 3, show inline confirmation with session names. One click confirms or cancels. No change to the underlying `signal_sessions` Tauri command.
**Effort:** S | **Priority:** P1 | **Target:** v0.7.0

### Dead Session Indicator (Send on Idle)
**What:** Block the Send button for idle sessions. Instead of silently dropping the message into the shell prompt, show a one-liner with the resume command and a copy button:
`Session ended. Resume it with: claude --resume <session-id>`
**Why:** Currently Send fires for all statuses. For idle sessions, Claude has exited. The message lands at the shell prompt and disappears. No error, no feedback. The user has no idea what happened.
**How:** In `SessionCard.tsx`, detect `session.status === "idle"` on Send click. Show inline callout with the session ID and a copy-to-clipboard button. Grey out or repurpose the Send button label to "Ended" for idle cards.
**Constraints:** No AppleScript, no resume automation. Just surface the right primitive and let the user run it in their terminal. Zero edge cases.
**Effort:** S | **Priority:** P1

### Session History Toggle ("Show All Sessions")
**What:** A "History" button in the dashboard header that loads sessions older than the 7-day `MAX_SESSION_AGE` gate. Currently 274 files on disk are invisible to the dashboard.
**Why:** Users lose access to older sessions with no indication they exist. The 7-day gate was a performance optimisation, not a product decision.
**How:** Add a `scan_all_sessions` Tauri command that calls `registry.scan_all` with no age cap. Button in header toggles between normal view and full history view. Idle-only sessions from history are visually dimmed to reduce noise.
**Tradeoffs:** This is a scroll problem, not a search problem. A user with 6 months of history will see hundreds of idle cards. Treat this as a stopgap until Semantic Session Search ships. Ship search before this if capacity allows.
**Effort:** M | **Priority:** P2

### Resume Primitive (Phase 3)
**What:** Proper "wake up idle session" flow: inject `claude --resume <session-id>` into the terminal tab, detect when Claude is ready (via JSONL sentinel), then inject the user's message.
**Why:** The natural follow-on to the Dead Session Indicator. Instead of copying the command, humOS handles the resume + message injection as a single atomic operation.
**Why not now:** Requires solving three hard problems: (1) JSONL collision: resume appends to the same file humOS is watching, new session ID may not be picked up; (2) startup race: no state gate between "Claude initialised" and "message inject", timer-based sync is fragile; (3) tab exclusivity: must verify cwd isn't already running another session before injecting. All three need the daemon's session tracking to do safely. This is `join()`-sized work.
**Depends on:** Daemon runtime model spec, Phase C app migration, `join()` primitive
**Effort:** L | **Priority:** P3

### Semantic Session Search
**What:** Search sessions by context and content, not just timestamp or project name.
**Why:** Finding the "office-hours design doc session" required grepping raw JSONL. Should be instant. Type "design doc" or "control room" and surface matching sessions.
**How:** Index session summaries (already generated via claude -p) + last_output snippets. Search across all sessions in ~/.claude/projects. Optional: embed summaries for semantic similarity.
**Effort:** M | **Priority:** P2

### Merge tmux + Claude Session Monitor
**What:** Link each Claude session card to the tmux pane it's running in. Add tmux-MCP server so Claude can read/write any terminal pane directly.
**Why:** v0.1.0 shows you what Claude is doing. tmux integration gives Claude eyes into your terminals. Together: two-way bridge, no more screenshots.
**Architecture:**
- tmux pane detection: match session cwd to tmux pane running directory
- tmux-mcp: Node.js MCP server with list_panes, read_pane, send_keys, watch_pane tools
- Card enhancement: show linked pane name, "Read Pane" action for Claude
**Effort:** L | **Priority:** P1 (core vision from design doc)

---

## v2.0 North Star: AI Agent Orchestration Primitives

The 10x product. Not a dashboard. An OS for AI agents.

### Primitive 1: pipe()
Route output from session A as input to session B automatically.
When session A completes or goes idle, inject its output/artifacts into session B's context.
No human copy-paste. Zero relay.
**Effort:** M | **Priority:** P1

### Primitive 2: signal()
Broadcast a message to all running sessions simultaneously.
"Abort." "New constraint: don't touch auth.ts." "Pivot. New direction:"
One message, all sessions receive it.
**Effort:** S | **Priority:** P1

### Primitive 3: join()
Wait for multiple sessions to complete, then aggregate their outputs.
"Tell me when sessions A, B, and C are all idle. Then summarize what they did."
Currently impossible. You have to watch each one manually.
**Effort:** M | **Priority:** P1

### Primitive 3.5: checkpoint (Design Doc)
**What:** Write `PLAN-checkpoint-primitive.md` specifying `checkpoint` as a first-class primitive, distinct from `PipeTrigger`. Define trigger vocabulary (OnIdle, OnFileWrite, OnExplicitMarker) and per-rule vs global semantics.
**Why:** `checkpoint` currently lives only as an attribute of pipe() via `PipeTrigger`. But join() needs the same concept to know "when is a session done enough to aggregate?" Two different primitives, same unsolved design question. If checkpoint is not specced independently before join() is built, the design will be re-solved twice and the solutions will diverge.
**How:** One-page markdown spec in repo root. No code changes. Scope: what constitutes a checkpoint, which primitives consume it, how explicit markers work alongside heuristic detection.
**Effort:** S | **Priority:** P2 | **Target:** Before v0.7.0 design begins

### Primitive 4: Orchestrator Session
A Claude session that monitors and coordinates all other sessions autonomously.
Detects WAITING states and routes them. Detects completions and triggers next steps.
You set the goal. It runs the pipeline. You review the output.
**Effort:** XL (L with CC) | **Priority:** P2

### Primitive 5: Task Compiler / DAG Executor
Describe a high-level goal. humOS decomposes it into parallel sub-sessions.
Manages dependencies: session C waits for A, session B runs immediately.
Aggregates outputs when all branches complete.
**Effort:** XL (L with CC) | **Priority:** P2

### Primitive 6: Persistent Cross-Session Memory (Project Brain)
Compressed, queryable store of decisions and constraints across all sessions.
New sessions get injected with relevant history automatically.
"We ruled out approach X in the Apr 8 session. Here's why."
**Effort:** M | **Priority:** P2

### Primitive 7: Reactive Workflows
Define triggers, not tasks.
"When build passes in session A, start deploy in session B."
"When any session is WAITING >5 min, notify me."
"When session C touches auth.ts, pause session D."
**Effort:** M | **Priority:** P2

---

## Proof of concept to validate the 10x
Build pipe() first. Take two sessions. When session A writes a file and goes idle,
automatically inject that file path into session B's context. No human in the loop.
If that works and feels right. The product is real.

---

## CEO Plan Gaps (added Apr 11)

### Rename the product
"Claude Control Room" fits the dashboard. Doesn't fit an AI agent OS.
Decide on a name that hints at coordination before v1.0 ships and it gets hard to rename.
**Effort:** S | **Priority:** P2

### Name the Anthropic risk + moat
If Anthropic ships native session orchestration, what's the differentiation?
Answer: local-first, works with existing subscription, no API costs for coordination layer, open source.
Cloud-hosted vs on-machine. Write this down explicitly in the plan.
**Effort:** S | **Priority:** P1

### Distribution decision
How does this reach other Claude Code power users?
Options: Homebrew tap, GitHub releases (.zip), landing page.
Not urgent for v0.1.0. Required before v1.0.
**Effort:** M | **Priority:** P2

### "Why now" framing
Claude Code adoption is accelerating. Multi-session workflows becoming normal.
Window to define this category is open now. Six months from now someone else will have built it.
Add this as urgency context to the CEO plan.
**Effort:** S | **Priority:** P2

### pipe() success criteria
Validated when: session A writes a schema, session B picks it up and writes tests, zero human relay.
That's the moment you know it's real. Write it as a concrete milestone, not a vague goal.
**Effort:** S | **Priority:** P1

---

## Naming Decision (Apr 11)

**Product name: humOS**
**Domain: humos.dev** (available, all variants clean)

Rationale: Sessions humming in the background while you do something else.
The human goes quiet. The work runs. OS signals infrastructure not a dashboard.
"Open" prefix dropped. GitHub presence signals open source without needing it in the name.

Competitors checked: Conductor (YC, $22M), opcode, claude-control. None use humOS.
Full domain sweep clean: humos.dev, humos.sh, humos.so, humos.build all available.

Action items before v1.0:
- Register humos.dev
- Rename GitHub repo: claude-control-room → humos
- Update app title in tauri.conf.json
- Update README and CLAUDE.md
- Design a mark that works as a menu bar icon (simple, monochrome)

---

## Items from /plan-eng-review (Apr 11)

### Rule Persistence
**What:** Persist pipe rules to ~/.humos/pipes.json, reload on app startup.
**Why:** Currently in-memory only. Rules are lost on every restart.
**How:** Serialize PipeManager.rules on every add/remove. Load on run().
**Effort:** S | **Priority:** P2 | **Target:** v0.3

### Terminal Emulator Support for inject_message
**What:** Detect terminal emulator (Terminal.app vs iTerm2 vs Warp vs Ghostty) and dispatch correct injection method.
**Why:** `do script` is Terminal.app-only. Other emulators will fail silently.
**How:** Check running process list, use iTerm2 AppleScript API for iTerm2, etc.
**Depends on:** inject_message rewrite (Issue 2A from eng review)
**Effort:** M | **Priority:** P1 (before v1.0 distribution)

### Startup Scan Performance
**What:** Add recency filter or file count cap to walkdir_recursive on startup.
**Why:** Users with 6+ months of Claude sessions may have thousands of JSONL files. Results in slow cold start.
**How:** Skip files with mtime > 30 days. Or limit to files modified in last 7 days.
**Effort:** S | **Priority:** P2 | **Target:** v0.3

### Adaptive Poll Interval (Phase C follow-on)
**What:** Slow the daemon poll from 5s to 30s when all sessions are idle.
**Why:** 5s polling when nothing is active wastes CPU and battery. If every session is idle, there's nothing useful to update.
**How:** After each poll, check if any session has status "running" or "waiting". If none, back off to 30s. Reset to 5s immediately when a running/waiting session appears.
**Effort:** S | **Priority:** P2 | **Target:** v0.5.x

### Daemon Version Handshake (Phase C follow-on)
**What:** Add `daemon_version: String` to the Health IPC response. App logs a warning when the version field doesn't match the compiled-in expected version.
**Why:** Daemon and app ship together in v0.5.0 but Homebrew updates could cause partial upgrades later. Without a handshake, a protocol mismatch silently discards IPC responses. The rescue path "log + discard" makes it invisible to the user.
**How:** Add `daemon_version` to `Response::Health`. App reads it on every health poll. If mismatch detected, surface in the daemon offline banner: "Daemon version mismatch. Restart daemon with: humos-daemon serve". Full handshake with negotiation is a larger follow-on.
**Effort:** S | **Priority:** P2 | **Target:** v0.5.x

---

## Agent Agnosticism: Multi-Agent Platform Vision

**Decision recorded:** humOS is NOT a Claude Code tool. It is an AI agent coordination OS.

Claude Code is the first supported agent because it writes structured JSONL to `~/.claude/projects/`. That was a convenient entry point. But the platform must support any coding agent that runs in a terminal, IDE, or CLI.

### Agents to support

| Agent | Session source | Detection method |
|-------|---------------|-----------------|
| Claude Code | `~/.claude/projects/*.jsonl` | Current (done) |
| Cursor | Process + workspace file | Watch `.cursor/` workspace state |
| Copilot (GitHub) | VS Code extension logs | Watch VS Code extension host logs |
| Aider | Terminal stdout | tmux pane watcher + stdout parser |
| Codex CLI | Process detection | Watch `~/.codex/` or stdout |
| Cline / Continue | VS Code extension | Extension state files |
| Devin / SWE agents | API/webhook | Webhook receiver, poll API |
| Custom agents | Stdin/stdout protocol | humOS agent SDK (see below) |

### humOS Agent SDK (Primitive 0)

Any agent that wants first-class support writes a `.jsonl` line to:
```
~/.humOS/sessions/<agent-name>/<session-id>.jsonl
```

Line format (minimal, agent-agnostic):
```json
{"type": "status", "sessionId": "...", "cwd": "...", "agent": "aider", "status": "running", "message": "...", "timestamp": "..."}
```

humOS watches `~/.humOS/sessions/` in addition to `~/.claude/projects/`. Any agent that emits this format gets a session card automatically. First-class pipe(), signal(), join() support included.

### Parser abstraction

Current `parser.rs` is Claude-specific. Refactor plan:
- `trait AgentParser { fn parse(path: &Path) -> Option<SessionState>; }`
- `ClaudeParser`: current JSONL format
- `HumOSParser`: generic SDK format (above)
- `AiderParser`: stdout line parser (future)
- Registry: match path pattern → use correct parser

### UI

- Session card gains `agent` badge (e.g. `claude`, `aider`, `cursor`) with agent-specific icon
- Filter bar: "All agents | Claude | Cursor | Aider | ..."
- Settings: toggle which agent directories to watch

### Priority

P1. This is the moat. No other tool (Conductor, opcode, claude-control) is agent-agnostic.
Being Claude-only is a ceiling. Being the coordination layer for ALL local agents is the 10x position.

---

## Post-signal() Roadmap (added 2026-04-11 by /autoplan)

### signal() v2: Selective Broadcast
Add session tagging/grouping so signal can target a subset of sessions ("abort backend agents, keep frontend running"). Requires SessionState.tags field and UI to assign tags. Design alongside join() since both need session grouping.
**Effort:** M | **Priority:** P2

### signal() v2: Signal Vocabulary
Define a vocabulary of structured signals (ABORT, PAUSE, CHECKPOINT, REDIRECT) that produce consistent agent behavior regardless of phrasing. Freeform text is powerful but inconsistent.
**Effort:** S | **Priority:** P2

### signal() v3: Programmatic API
Make signal_sessions callable from other sessions (pipe → signal chain). Enables agent-to-agent signaling without human in the loop. The real primitive for an AI OS.
**Effort:** M | **Priority:** P1

### signal() v3: File-based signaling
Write to ~/.humOS/signals.json watched by sessions, as an alternative to AppleScript injection. More reliable, works with non-terminal agents (browser agents, API agents). Requires agents to be primed to watch.
**Effort:** M | **Priority:** P2

### Signal Audit Log
**What:** Append every signal broadcast to `~/.humOS/signal-log.db` (SQLite): timestamp, message text, target session IDs, success/fail per session.
**Why:** `SignalFiredEvent` captures delivery in memory only. When a broadcast misfires or produces unexpected downstream behavior, there is no way to reconstruct what was sent and to whom. The audit log is the debugging primitive for signal() and the first structured event store the observability roadmap can build on.
**How:** New `signal_log.rs` module. On every `signal_sessions` call, after delivery, INSERT one row. Schema: `(id INTEGER PRIMARY KEY, ts TEXT, message TEXT, target_count INT, success_ids TEXT, fail_ids TEXT)`. Expose a read-only Tauri command for a future log panel.
**Effort:** M | **Priority:** P2 | **Target:** v0.7.1

### signal() scale: Parallel injection
For N>15 sessions, spawn tokio tasks for parallel AppleScript calls. ~2s sequential latency becomes ~200ms parallel.
**Effort:** S | **Priority:** P3

### Opt-In Anonymous Telemetry (Path B, approved Apr 12)
**Decision:** Add opt-in anonymous telemetry to measure actual product usage.
**North star metric:** Weekly pipe() fires per active user.
**What to track:** app opens, pipe rules created, pipe fires, signal broadcasts, session count. Counts only, no content, no file paths, no personal data.
**UX:** First-launch prompt: "Help improve humOS? Anonymous usage stats only. No session content." Accept / Decline. Decline = no telemetry ever. Accept = anonymous counts sent to a simple endpoint.
**Dashboard metrics:**
- Weekly installs (Homebrew + .zip)
- Weekly active users (app opens)
- Pipe rules created (total)
- Pipe fires per week
- Signal broadcasts per week
- GitHub stars
**Backend:** Simple POST to a Cloudflare Worker or Val.town function. Store in a free Turso/Supabase DB. No auth, no user IDs, just event counts with a random install ID.
**Effort:** M | **Priority:** P1

### Strategic: humOS Runtime Model
Before shipping join() and orchestrator sessions, define the runtime model: does humOS have a scheduler and message bus, or is it a GUI layer on ad-hoc sessions? These are different architectures. Recommended: define the runtime contract as a spec before building join().
**Effort:** S (spec), XL (build) | **Priority:** P1

---

## Coordination Proof Instrumentation

These items exist to make the value of coordination primitives measurable and visible. They are not a cost product. Token data surfaces as evidence per pipe edge, not as a billing dashboard. Alignment rationale: the latent demand framework says users adopt primitives when they can see the value. These items make the value visible.

### Token Instrumentation in Parser
**What:** Extend `RawLine` in `parser.rs` to deserialize `message.model`, `message.usage.input_tokens`, `message.usage.output_tokens`, `message.usage.cache_read_input_tokens`, and `message.usage.cache_creation_input_tokens`. Accumulate per-session totals in `SessionState`.
**Why:** Every assistant turn in the JSONL already contains full token and cache data. The parser reads `message` as a raw `Value`, drills into `content[]`, and discards `usage` and `model` entirely. These fields are siblings of `content` inside `message`. No new data source is needed. This is the measurement foundation for proving that pipe() and signal() reduce wasted coordination tokens.
**How:** Add `usage: Option<UsageData>` and `model: Option<String>` to `RawLine`. Add `input_tokens: u64`, `output_tokens: u64`, `cache_read_tokens: u64`, `cache_creation_tokens: u64`, `model: String` to `SessionState`. Accumulate with `+=` per assistant line in the parser loop.
**Constraint:** Token data is used to show coordination value per pipe edge, not as a standalone cost dashboard. Do not build a cost UI before at least one distilled payload mechanism exists.
**Effort:** S | **Priority:** P2 | **Target:** v0.7.1

### Per-Pipe Token Savings Display
**What:** Show token delta per pipe edge in the dashboard: "This pipe() saved ~2,800 tokens vs raw transcript handoff." Visible as a label on the pipe edge line or as a tooltip on the rule card.
**Why:** Users cannot see why pipe() is valuable until they see a number. This is the Trojan horse from the latent demand framework: an expressed metric (token count) that reveals the latent value (coordination reduces waste). The moment a user sees "91% fewer tokens on this handoff" the primitive clicks.
**Depends on:** Token Instrumentation in Parser + at least one distilled payload mechanism shipping first.
**Effort:** M | **Priority:** P3 | **Target:** Post v0.7.0