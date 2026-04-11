
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
Describe a high-level goal. Control Room decomposes it into parallel sub-sessions.
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

**Product name: HumOS**
**Domain: humos.dev** (available, all variants clean)

Rationale: Sessions humming in the background while you do something else.
The human goes quiet. The work runs. OS signals infrastructure not a dashboard.
"Open" prefix dropped — GitHub presence signals open source without needing it in the name.

Competitors checked: Conductor (YC, $22M), opcode, claude-control — none use HumOS.
Full domain sweep clean: humos.dev, humos.sh, humos.so, humos.build all available.

Action items before v1.0:
- Register humos.dev
- Rename GitHub repo: claude-control-room → humos
- Update app title in tauri.conf.json
- Update README and CLAUDE.md
- Design a mark that works as a menu bar icon (simple, monochrome)
