
## Feature Ideas (Backlog)

### Semantic Session Search
**What:** Search sessions by context and content, not just timestamp or project name.
**Why:** Finding the "office-hours design doc session" required grepping raw JSONL. Should be instant — type "design doc" or "control room" and surface matching sessions.
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

## v2.0 North Star — AI Agent Orchestration Primitives

The 10x product. Not a dashboard. An OS for AI agents.

### Primitive 1: pipe()
Route output from session A as input to session B automatically.
When session A completes or goes idle, inject its output/artifacts into session B's context.
No human copy-paste. Zero relay.
**Effort:** M | **Priority:** P1

### Primitive 2: signal()
Broadcast a message to all running sessions simultaneously.
"Abort." "New constraint: don't touch auth.ts." "Pivot — here's the new direction."
One message, all sessions receive it.
**Effort:** S | **Priority:** P1

### Primitive 3: join()
Wait for multiple sessions to complete, then aggregate their outputs.
"Tell me when sessions A, B, and C are all idle — then summarize what they did."
Currently impossible. You have to watch each one manually.
**Effort:** M | **Priority:** P1

### Primitive 4: Orchestrator Session
A Claude session that monitors and coordinates all other sessions autonomously.
Detects WAITING states and routes them. Detects completions and triggers next steps.
You set the goal. It runs the pipeline. You review the output.
**Effort:** XL (L with CC) | **Priority:** P2

### Primitive 5: Task Compiler / DAG Executor
Describe a high-level goal. humOS decomposes it into parallel sub-sessions.
Manages dependencies — session C waits for A, session B runs immediately.
Aggregates outputs when all branches complete.
**Effort:** XL (L with CC) | **Priority:** P2

### Primitive 6: Persistent Cross-Session Memory (Project Brain)
Compressed, queryable store of decisions and constraints across all sessions.
New sessions get injected with relevant history automatically.
"We ruled out approach X in the Apr 8 session — here's why."
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
If that works and feels right — the product is real.

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
Options: Homebrew tap, GitHub releases (.dmg), landing page.
Not urgent for v0.1.0 — required before v1.0.
**Effort:** M | **Priority:** P2

### "Why now" framing
Claude Code adoption is accelerating. Multi-session workflows becoming normal.
Window to define this category is open now — 6 months from now someone else will have built it.
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
"Open" prefix dropped — GitHub presence signals open source without needing it in the name.

Competitors checked: Conductor (YC, $22M), opcode, claude-control — none use humOS.
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
**Why:** Currently in-memory only — rules are lost on every restart.
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
**Why:** Users with 6+ months of Claude sessions may have thousands of JSONL files — slow cold start.
**How:** Skip files with mtime > 30 days. Or limit to files modified in last 7 days.
**Effort:** S | **Priority:** P2 | **Target:** v0.3

---

## Agent Agnosticism — Multi-Agent Platform Vision

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
- `ClaudeParser` — current JSONL format
- `HumOSParser` — generic SDK format (above)
- `AiderParser` — stdout line parser (future)
- Registry: match path pattern → use correct parser

### UI

- Session card gains `agent` badge (e.g. `claude`, `aider`, `cursor`) with agent-specific icon
- Filter bar: "All agents | Claude | Cursor | Aider | ..."
- Settings: toggle which agent directories to watch

### Priority

P1 — this is the moat. No other tool (Conductor, opcode, claude-control) is agent-agnostic.
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

### signal() scale: Parallel injection
For N>15 sessions, spawn tokio tasks for parallel AppleScript calls. ~2s sequential latency becomes ~200ms parallel.
**Effort:** S | **Priority:** P3

### Opt-In Anonymous Telemetry (Path B — approved Apr 12)
**Decision:** Add opt-in anonymous telemetry to measure actual product usage.
**North star metric:** Weekly pipe() fires per active user.
**What to track:** app opens, pipe rules created, pipe fires, signal broadcasts, session count. Counts only, no content, no file paths, no personal data.
**UX:** First-launch prompt: "Help improve humOS? Anonymous usage stats only. No session content." Accept / Decline. Decline = no telemetry ever. Accept = anonymous counts sent to a simple endpoint.
**Dashboard metrics:**
- Weekly installs (Homebrew + .dmg)
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