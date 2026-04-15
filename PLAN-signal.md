# Plan: signal() — Broadcast to All Running Sessions

**Branch:** main | **Feature:** signal() | **Priority:** P1 (Primitive 2)

---

## Problem Statement

Right now, if you have 5 Claude sessions running in parallel and you discover a new constraint ("don't touch auth.ts") or need to pivot all of them ("new direction: focus on the API"), you have to:
1. Open each Terminal tab manually
2. Type or paste the message into each one individually
3. Wait for each session to receive and process it

With `signal()`, you click once in humOS and all running sessions receive the broadcast simultaneously. "Abort." "New constraint: don't touch auth.ts." "Pivot — here's the new direction." One message, all sessions, zero relay.

---

## Premises

1. All running sessions are reachable via the same `inject_message` AppleScript mechanism already used by the Send button.
2. "Running" means `status === "running"` — sessions where Claude is actively working.
3. The user wants to broadcast to ALL running sessions, not selected ones (v1). Selection comes later.
4. The message should appear in the UI when it fires (activity log entry).
5. A simple broadcast button in the header is sufficient UX for v1. No session selection needed.

---

## Scope

### In scope
- `signal_sessions` Tauri command (Rust): iterate all non-idle sessions (running + waiting), call `inject_message` for each
- Signal button in the humOS header (next to Pipes)
- Signal input UI: click button → text field appears → enter message → Enter to send
- Activity log entry when signal fires: "signal → N sessions: [message preview]"
- Canvas animation: brief flash on ALL running session cards simultaneously (vs pipe's A→B line)
- Error handling: partial failure (some sessions got it, some didn't) — report which failed

### Out of scope (defer to TODOS.md)
- Session selection (broadcast to subset of sessions)
- Scheduled signals ("send this in 30 minutes")
- Signal history / audit log
- Signal templates
- Signal from CLI (outside humOS UI)

---

## Technical Design

### Backend (Rust — `src-tauri/src/lib.rs`)

New Tauri command:
```rust
#[tauri::command]
async fn signal_sessions(
    message: String,
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<SignalResult>, String> {
    let sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
    let running: Vec<SessionState> = sessions
        .values()
        .filter(|s| s.status != "idle")
        .cloned()
        .collect();
    drop(sessions);

    let mut results = Vec::new();
    for session in &running {
        let result = inject_message(&session.cwd, &message);
        results.push(SignalResult {
            session_id: session.id.clone(),
            project: session.project.clone(),
            success: result.is_ok(),
            error: result.err(),
        });
    }

    // Emit event for UI activity log + animation
    app.emit("signal-fired", SignalFiredEvent {
        message: message.clone(),
        success_ids: results.iter().filter(|r| r.success).map(|r| r.session_id.clone()).collect(),
        fail_ids: results.iter().filter(|r| !r.success).map(|r| r.session_id.clone()).collect(),
        success_count: results.iter().filter(|r| r.success).count(),
        fail_count: results.iter().filter(|r| !r.success).count(),
    }).ok();

    Ok(results)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SignalResult {
    session_id: String,
    project: String,
    success: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SignalFiredEvent {
    message: String,
    session_ids: Vec<String>,
    success_count: usize,
    fail_count: usize,
}
```

Register in `lib.rs` plugin builder alongside existing commands.

### Frontend (React/TypeScript)

**`App.tsx` changes:**
- Add `signalOpen: boolean` state + `signalMessage: string` state + `signalPending: boolean` + `signalFlashIds: Set<string>` + `signalFailIds: Set<string>`
- Signal button in header: `⌁ Signal`, disabled (greyed out) when 0 non-idle sessions exist
- Active state: same treatment as Pipes button when open (border-color: var(--signal), color: var(--signal))
- Command bar: 40px overlay anchored below header, full width, auto-dismissed by Escape or click-outside
  - Input placeholder: "Broadcast to all running sessions..."
  - Character limit: 512, counter shown at 80% capacity
  - Empty input: Enter disabled (send button greyed, Enter key no-op)
  - In-progress: input read-only, Signal button pulsing, `signalPending: true`
- 2-second undo window: on Enter, set a 2s timeout before actual `invoke("signal_sessions")`. Show toast: "Sending to N sessions — Cancel". On cancel, clear timeout and close.
- On signal-fired event: set `signalFlashIds` (success) and `signalFailIds` (failed) → clear after 800ms
- All-fail state: command bar stays open, red border, inline text: "Signal failed — no sessions received it"
- Activity log entry: `"⌁ signal → N sessions: [first 40 chars of message]"`
- `handleSignal()`: calls `invoke("signal_sessions", { message: message.trim() })`

**`SessionCard.tsx` changes:**
- Accept `signalSuccess?: boolean` and `signalFail?: boolean` props
- `signalSuccess`: green ripple flash (staggered by card index * 80ms from signal button origin)
- `signalFail`: red glow for 1s then fade

**`index.css` changes:**
- `.session-card--signal-success`: green glow keyframe (distinct color/timing from pipe target flash)
- `.session-card--signal-fail`: red glow keyframe
- `.signal-command-bar`: full-width overlay bar below header, dark surface, 40px height
- `.signal-command-bar--error`: red border variant

**Event flow:**
```
User types message → presses Enter
  → invoke("broadcast_signal", { message })
    → Rust iterates running sessions
    → inject_message() for each
    → emits "signal-fired" event
  → App.tsx listener fires
    → adds activity log entry
    → sets flashingSessionIds (setTimeout to clear after 800ms)
    → SessionCards with matching IDs receive isSignalTarget=true → flash
```

---

## Test Plan

- Unit: `broadcast_signal` with 0 running sessions (no-op, returns empty vec)
- Unit: `broadcast_signal` with 1 running, 1 idle — only running gets injected
- Unit: partial failure handling (one inject_message fails, others succeed)
- Integration: `signal-fired` event shape matches frontend listener expectations
- Visual: flash animation fires on correct cards, clears after timeout
- Visual: activity log shows "signal → N sessions: [message]"

---

## Effort

**S** — ~2-3 hours. The injection mechanism (`inject_message`) already works. This is:
1. One new Rust command (~40 lines)
2. One new struct pair (SignalResult, SignalFiredEvent)
3. ~60 lines of React (button, input, event handler, flash prop threading)
4. ~20 lines of CSS (flash keyframe)

No new infrastructure. No new AppleScript. Builds directly on what pipe() proved out.

---

## Decision Audit Trail

<!-- AUTONOMOUS DECISION LOG -->

| # | Phase | Decision | Classification | Principle | Rationale | Rejected |
|---|-------|----------|----------------|-----------|-----------|---------|
| 1 | CEO/Premise | Include waiting sessions (status != "idle") | Mechanical | P1 | Waiting sessions need interruption as much as running ones | Running-only |
| 2 | CEO/Design | Rename to `signal_sessions` | Mechanical | P5 | Clearer verb-noun, more composable | `broadcast_signal` |
| 3 | CEO/Strategic | Defer runtime model question to post-signal() | Mechanical | P6 | signal() is S-effort; runtime model is XL. Don't block. | Block signal() |
| 4 | CEO/Sec3 | Add `successIds` to SignalFiredEvent | Mechanical | P1 | Partial delivery without per-session result is misleading | Single boolean |
| 5 | CEO/Sec3 | Capture file-based signaling in TODOS.md | Mechanical | P4 | Good idea, out of blast radius | Add to v1 |
| 6 | CEO/Sec5 | Add all-fail error toast | Mechanical | P1 | Activity log only is insufficient for all-fail | Log only |
| 7 | Design/D1 | Command bar overlay below header | Mechanical | P5 | Same pattern as Pipes drawer, inverted axis | Inline header |
| 8 | Design/D2 | Disable Signal button when 0 non-idle sessions | Mechanical | P1 | Zero-state is critical — must be handled | Always enabled |
| 9 | Design/D3 | In-progress: input read-only during injection | Mechanical | P1 | No loading state = UI feels broken | No state |
| 10 | Design/D4 | All-fail: inline error in command bar | Mechanical | P1 | Activity log too small for failure communication | Log only |
| 11 | Design/D5 | Partial fail: red glow on failed cards | Mechanical | P1 | Green flash on failed sessions is misleading | All green |
| 12 | Design/D6 | Ripple-origin stagger for broadcast flash | Taste | P5 | Distinguishes broadcast from pipe visually | Color-only |
| 13 | Design/D7 | 2-second undo toast | Mechanical | P1 | Accidental send to N AI agents is high consequence | Confirm dialog |
| 14 | Design/D8 | 512-char limit with counter at 80% | Mechanical | P1 | Unbounded input is a latent bug | No limit |
| 15 | Design/D9 | Escape + click-outside closes command bar | Mechanical | P5 | Standard UX expectation | Sticky |
| 16 | Design/D10 | `⌁ Signal` glyph + active state treatment | Mechanical | P5 | Consistent with waveform-heavy aesthetic | Plain text |
| 17 | DX/1 | Activity log names failed sessions by project name | Mechanical | P1 | "signal failed: proj-a, proj-b" is actionable; IDs are not | IDs only |
| 18 | DX/2 | Tooltip on disabled Signal button | Mechanical | P5 | "No running sessions" — 5-minute fix | No tooltip |
| 19 | DX/3 | Add signal() section to README | Mechanical | P1 | DX completeness — users need to know the feature exists | Skip docs |

## Success Criteria

1. User can broadcast a message from humOS header to all running sessions in <2 seconds
2. All running sessions receive the message via AppleScript injection
3. Activity log shows which sessions got it and any failures
4. Session cards flash visually when they receive a signal
5. Works correctly with 0, 1, and 5+ running sessions

---

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/autoplan` | Scope & strategy | 1 | clean | 6 fixed, 6 deferred to TODOS.md |
| Eng Review | `/autoplan` | Architecture & tests | 1 | clean | 14 tests specified, 0 critical gaps |
| Design Review | `/autoplan` | UI/UX gaps | 1 | resolved | 10 findings → 8 auto-fixed, 1 taste |
| DX Review | `/autoplan` | Developer experience | 1 | clean | 7.75/10, TTHW 30s, 3 gaps fixed |
| Codex Review | — | Independent 2nd opinion | 0 | — | unavailable |

**VERDICT:** APPROVED — 19 decisions (18 auto, 1 taste: ripple-origin flash chosen). Ready to build.

**Approved:** 2026-04-11
