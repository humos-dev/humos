# humOS Discovery Notes

---

## Round 1: May 2026 (2 users, in-the-wild usage)

**Users:** Jonathan Atiene (developer, active pipe user), Sajioloye (developer, prospective user)
**Source:** Direct conversation (WhatsApp), April 2026 brainstorm session
**Context:** Neither user was recruited for research. Both gave feedback unprompted during normal usage or conversation.

---

### Finding 1: Memory beats parallelism (adoption blocker)

**Source:** Jonathan Atiene
**Quote:** "I mostly use one session because of memories."

Jonathan runs Claude Code for coding work and has deliberately chosen to stay in a single long session rather than split across multiple sessions. His reason is not laziness or unfamiliarity with humOS. It is a considered trade-off: one long session preserves accumulated context; multiple sessions fragment it.

This is the core adoption barrier for humOS. The multi-session pitch only works if users believe the coordination value exceeds the memory cost. Jonathan has not crossed that line.

Project Brain ribbon is the intended answer to this objection. But it has not effectively shipped to users. The ribbon requires the `humos-daemon` binary to be running. The daemon is not included in the ZIP distribution. It requires a separate manual step (`scripts/install-daemon.sh`) that is not part of the default install flow. Users who install humOS from ZIP, including Jonathan, never see the ribbon. The feature exists in the code. It is invisible in the product.

**Priority:** P0. The answer to the core adoption barrier exists but users cannot reach it. This is a distribution problem, not a product problem. Until the daemon ships as part of the standard install, the memory objection has no answer in the hands of real users.

**Implication for product:** Two actions needed. First: include the daemon in the ZIP install and run `install-daemon.sh` automatically on first launch, or bundle daemon auto-start into the app itself. Second: once the daemon is reachable, surface the memory model explanation at first pipe rule creation: "Session B will receive context from Session A automatically."

---

### Finding 2: Autonomous behaviour is invisible and surprising

**Source:** Jonathan Atiene
**What happened:** Jonathan woke his laptop after sleeping it, resumed coding, and noticed a new node had opened. He did not know if this was a bug or expected humOS behaviour. Bolu confirmed it was a pipe firing on idle. Jonathan had no way to know this had happened, when it happened, or what was injected.

The behaviour was correct. The invisibility was the problem.

**Two gaps this reveals:**

1. No "while you were away" summary. When a user returns to their machine after absence, humOS should surface a brief log of what fired: "1 pipe fired at 10:34pm. Session A injected into Session B." Currently there is nothing.

2. No persistent audit trail. Jonathan cannot reconstruct what happened. If the injected context caused unexpected downstream behaviour in Session B, he has no way to diagnose it.

**Priority:** P1. Users who cannot see what the product did autonomously will not trust it enough to set up more rules.

**Implication for product:** Signal audit log (already in TODOS.md) directly addresses this. Add "while you were away" summary banner on app open as a companion.

---

### Finding 3: Output quality is unproven

**Source:** Jonathan Atiene
**Quote:** "Quality of output. I need to also observe."

Jonathan has pipe rules configured but is in wait-and-see mode. He has not seen a moment where the pipe demonstrably produced better output than his single-session workflow would have. Until that moment happens and is visible to him, multi-session is a cost he is bearing (fragmented memory, unexpected autonomous actions) without a proven benefit.

**Priority:** P1. The product needs to manufacture the "aha moment": a case where the pipe visibly did something useful that a single session could not have done alone. This is not a product change; it is an onboarding and demo problem.

**Implication for product:** The demo video planned for v0.6.0 (Claude + opencode side by side, signal() broadcast) is the right vehicle. The demo must show output quality, not just the mechanic of the broadcast.

---

### Finding 4: Token costs are a real purchase-decision input

**Source:** Sajioloye
**Signal:** Flagged token cost concern twice in the same conversation, unprompted.
**Quote (paraphrased):** Told Bolu to verify any cost savings claims before putting them on the homepage.

Sajioloye is not a current user. He is a developer evaluating whether humOS is worth adopting. Token cost came up twice without being prompted, which means it is a real filter in his decision. He is not skeptical of humOS. He is asking for proof before committing.

This is a purchase-decision input, not a current adoption blocker. It becomes relevant when humOS has a public launch and cost claims go on the homepage.

**Priority:** P2. Needs verification before v0.6.0 marketing copy is written. Do not claim cost savings without a number. One measurable benchmark beats ten vague claims.

**Implication for product:** Token instrumentation in the parser (already in TODOS.md) is the prerequisite. Once token data is in SessionState, run one concrete benchmark: same task, with pipe() distilled handoff vs raw transcript handoff. Publish the delta.

---

## Common thread across both users

Both users are engaged and not churning. Both are blocked by the same underlying problem: humOS does not make its own value visible at the moment it matters.

Jonathan cannot see what the pipe did while he slept. Sajioloye cannot see whether the cost claims are real. The product works but it is opaque. Opening the product to itself (audit log, provenance headers, token instrumentation) converts both users from "I am observing" to "I am convinced."

---

## Open questions for next round of discovery

1. Is the memory objection universal? Do all developers who care about context quality make the same single-session trade-off, or is this Jonathan-specific?
2. What would make Jonathan recommend humOS to another developer today? What is missing from that story?
3. Does Sajioloye's cost concern represent the broader ICP? Or is his profile (cost-conscious, needs proof) a minority within the developer-running-5-sessions persona?
4. What does a "successful" pipe firing look like to Jonathan? Has he seen one yet?

---

## Action items from this round

| Finding | Action | TODOS.md item | Priority |
|---------|--------|---------------|----------|
| Memory beats parallelism | Bundle daemon in ZIP so Brain ribbon works on fresh install (answers memory objection structurally). Memory model copy in onboarding is a follow-on once ribbon is reachable. | Bundle Daemon in ZIP (TODOS, P0) | P0 |
| Autonomous behaviour invisible | "While you were away" banner on app open | Signal Audit Log (v0.6.1) | P1 |
| Output quality unproven | Demo video must show output quality, not just mechanics | v0.6.0 scope | P1 |
| Token costs need proof | Run one benchmark before writing cost copy | Token Instrumentation (v0.6.1) | P2 |
