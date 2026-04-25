# Design System — humOS

**Last updated:** 2026-04-25
**Created by:** /design-consultation

---

## Product Context

- **What this is:** Native macOS Tauri app (dark only) for developers coordinating multiple Claude CLI sessions
- **Who it's for:** Developers running 3-20 parallel AI agent sessions who want Unix-style coordination primitives (pipe, signal, join)
- **Space/industry:** Developer tools, AI agent orchestration, terminal tooling
- **Project type:** Native macOS app / monitoring dashboard / infrastructure tool

---

## Aesthetic Direction

- **Direction:** Industrial Minimal
- **Decoration level:** Minimal (typography and color do all the work)
- **Mood:** Built-by-the-kernel-team energy. The UI should feel like infrastructure, not an app. htop's clarity, Linear's precision, Instruments' data density. No gradients, no decorative radius, no chrome that doesn't earn its pixels.
- **Reference products:** Activity Monitor, htop, Linear (dark), Warp terminal
- **Anti-patterns to avoid:** Rounded consumer-app cards, purple gradients, icon grids, decorative blobs, centered-everything layouts

---

## Typography

**Rule: JetBrains Mono everywhere. No display font. No system font for rendered text.**

This is a deliberate risk. Most dev dashboards use Inter or system fonts. Monospace everywhere signals "terminal layer" — humOS lives one level above the shell, and the typography should show that. Users who run 10 parallel Claude sessions in Terminal will see this and feel understood.

- **All UI text:** JetBrains Mono 400/500/600
- **Data / IDs / code:** JetBrains Mono (same family, tabular-nums where applicable)
- **Loading:** Google Fonts — `https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@300;400;500;600;700&display=swap`
- **Fallback:** `"JetBrains Mono", ui-monospace, monospace`
- **System font:** Never for rendered text. Only acceptable as a CSS fallback in the stack.

**Type scale:**

| Role | Size | Weight | Usage |
|------|------|--------|-------|
| App title | 14px | 600 | "humOS" in header |
| Section heading | 13px | 600 | "3 sessions detected" |
| Card project name | 12px | 600 | Session card titles |
| Body / descriptions | 11px | 400 | Activity log, output preview |
| UI labels / status | 10px | 500 | "RUNNING", "WAITING", "IDLE" — uppercase + letter-spacing |
| Meta / timestamps | 10px | 400 | Modified at, tool count |
| Badges / tags | 9px | 600 | Provider badges, tool counts |

---

## Color

**Approach:** Restrained with strict semantic rules. Color is information, not decoration.

**Core semantic rule:** Each color has exactly one meaning. Never use a color outside its semantic role.

| Token | Hex | Meaning | When to use |
|-------|-----|---------|-------------|
| `--signal` | `#3ecf8e` | Session health / alive | Running dot, active border glow, waveform bars |
| `--coord` | `#3b82f6` | Coordination active | Pipe edges, pipe dots on cards, signal broadcast icon, pipe history text |
| `--amber` | `#f59e0b` | Waiting for input | Waiting status dot only |
| `--error` | `#f87171` | Error / failure | Action errors, dead session callout, failed pipe |
| Green NEVER used for coordination. Blue NEVER used for session health. | | | |

**Background scale (near-black, strictly layered):**

| Token | Hex | Usage |
|-------|-----|-------|
| `--bg` | `#080808` | App background |
| `--bg-1` | `#0b0b0b` | Subtle variation |
| `--bg-2` | `#0d0d0d` | Header, footer, drawers |
| `--surface` | `#111111` | Session cards, modals |
| `--border` | `#1a1a1a` | Default borders |
| `--border-2` | `#262626` | Hover/active borders |
| `--grid-line` | `#0e0e0e` | Background grid (see Grid section) |

**Text scale:**

| Token | Hex | Usage |
|-------|-----|-------|
| `--text` | `#e8e8e8` | Primary content |
| `--text-1` | `#bdbdbd` | Secondary content |
| `--text-2` | `#999999` | Muted labels |
| `--text-3` | `#666666` | Placeholder, metadata |

---

## Grid Background

**Rule:** The app background renders subtle grid lines.

The grid reinforces the "coordinate space" mental model — sessions are plotted on a surface, pipe edges connect points on the grid. No other product in the agent coordination space has this. It makes a screenshot of humOS immediately identifiable.

```css
background-image:
  linear-gradient(var(--grid-line) 1px, transparent 1px),
  linear-gradient(90deg, var(--grid-line) 1px, transparent 1px);
background-size: 24px 24px;
```

**Where it applies:** App main background (`--bg`), card grid area.
**Where it does NOT apply:** Session card surfaces, modals, drawers (these use `--surface` / `--bg-2`).
**Performance:** Static CSS background — no JS, no canvas. Negligible cost.

---

## Spacing

- **Base unit:** 8px
- **Density:** Compact. Monitoring tools are dense by nature. Users with 10+ cards need information, not breathing room.
- **Card padding:** 12px (was 16px — reduced to feel like data panels, not app cards)

**Scale:**

| Token | Value | Primary usage |
|-------|-------|---------------|
| `xs` | 4px | Icon gaps, dot spacing |
| `sm` | 8px | Element gaps within components |
| `md` | 12px | Card padding, component internal padding |
| `lg` | 16px | Section gaps, header padding |
| `xl` | 24px | Major layout gaps |
| `xxl` | 48px | Page-level vertical rhythm |

---

## Border Radius

**Rule:** Small, precise. Not consumer-app bubbly. Infrastructure tools use tight radius.

| Token | Value | Usage |
|-------|-------|-------|
| `r-sm` | 3px | Badges, tags, small inline elements |
| `r-md` | 5px | Session cards (primary), drawers, inputs |
| `r-lg` | 6px | Large panels, modals |

**Do not use:** 8px or larger for cards (previous value — too soft). No `border-radius: 9999px` except for circular status dots.

---

## Pipe Edges (Defining Visual Element)

The persistent pipe edges between connected session cards are the defining visual identity of humOS. They should be immediately recognizable in any screenshot.

**Rendering:** Canvas overlay (`position: fixed`, full viewport, `pointer-events: none`, `z-index` above cards).

**Styles:**

| State | Color | Line style | Arrowhead |
|-------|-------|------------|-----------|
| Active connection (at least one session non-idle) | `rgba(59, 130, 246, 0.35)` | Solid, 1px | Yes, at target end |
| Idle connection (both sessions idle) | `rgba(100, 100, 100, 0.25)` | Dashed (4px gap 6px), 1px | No |
| Fire animation (on top of static edge) | `#3ecf8e` pulse | Animated dashed line | Travelling dot |

**Behavior:**
- Static edges visible at all times when pipe rules exist (not just during fire)
- Skip drawing if either card has zero bounding rect (off-screen)
- Redraw after: window resize, session list change (50ms delay for DOM), after fire animation completes
- Fire animation draws on top of static edge via `drawBackground` callback — edges never disappear during animation

---

## Motion

**Approach:** Minimal-functional. One expressive exception.

**Rule:** The pipe fire animation is the ONLY place motion is expressive. Everything else is purely functional.

| Context | Duration | Easing | Notes |
|---------|----------|--------|-------|
| State transitions (border-color, opacity) | 100-150ms | ease | Hover, active, focus |
| Card signal flash | 800ms | — | CSS class toggle only |
| Card signal fail flash | 1000ms | — | CSS class toggle only |
| Pipe fire animation | 500ms travel + 150ms hold + 600ms fade | ease-out cubic | The one expressive moment |
| Waveform bars | 1.2s | ease-in-out | Infinite, staggered |
| Status dot breathe (running) | 2s | ease-in-out | Infinite |

**Never use:** Spring physics, bounce easing, scroll-driven animations, entrance animations for cards.

---

## Layout

- **Approach:** Grid-disciplined. Sessions in `auto-fill, minmax(340px, 1fr)` grid.
- **Max content width:** None (full width — monitoring tools use screen real estate)
- **Header:** Fixed top, `--bg-2`, left = wordmark + session count, right = action buttons
- **Footer:** Activity log, fixed bottom, `--bg-2`
- **Card gap:** 8px

---

## Component Patterns

### Session Card
- Background: `--surface` (#111111)
- Border: `1px solid --border` at rest, `rgba(62, 207, 142, 0.25)` when running, `rgba(59, 130, 246, 0.25)` when pipe-connected
- Padding: 12px
- Radius: 5px
- Min-height: 160px
- Structure (top to bottom): BrainRibbon (if applicable) → header (project + cwd + pipe dot) → status row (dot + label + tool badge + timestamp) → last output → pipe history (if pipe has fired) → actions (hover-reveal) → send input / dead session callout

### Pipe Connection Dot
- 5px circle, `--coord` (#3b82f6), opacity 0.7
- Positioned top-right of card header
- Shown on both source and target cards

### Pipe History
- Card footer line: "pipe → [target]" (source) or "received from [source] · N min ago" (target)
- Font: 9px, `--coord` for the session name, `#444` for the rest
- Separated from card body by `border-top: 1px solid --border`

### Dead Session Indicator
- Shown when user clicks Send on an idle session (replaces send input)
- Background: `rgba(248, 113, 113, 0.06)`, border: `rgba(248, 113, 113, 0.2)`
- Text: "Session ended. Resume with: `claude --resume <id>`" + copy button
- Send button relabeled "Ended" for idle cards (muted style)

### Status Dots
| Status | Color | Animation |
|--------|-------|-----------|
| Running | `#3ecf8e` | Breathe (2s, opacity + scale) |
| Waiting | `#f59e0b` | None |
| Idle | `#444444` | None |

### Activity Log
- Fixed footer, `--bg-2`, `border-top: 1px solid --border`
- Font: 10px, `--text-3` base, `--text-2` for active entries
- Pipe entries: `--coord` arrow icon
- Signal entries: `--coord` ⌁ icon
- Max entries displayed: 5 (stored: 20 in localStorage)
- Opacity fade: `1 - i * 0.18` (most recent = full opacity)

### Buttons
- Font: JetBrains Mono, 10px, uppercase, letter-spacing 0.06em
- Radius: 3px
- Three variants:
  - Primary (session action): `rgba(62, 207, 142, 0.08)` bg, `rgba(62, 207, 142, 0.3)` border, `--signal` text
  - Coordination (pipes/signal): `rgba(59, 130, 246, 0.08)` bg, `rgba(59, 130, 246, 0.3)` border, `--coord` text
  - Ghost: transparent bg, `--border-2` border, `--text-2` text

---

## BrainRibbon

The ambient project context strip at the top of session cards.

- Margin: `-12px -12px 0 -12px` (bleeds to card edges, updated from -16px)
- Padding: 5px 10px
- Background: `rgba(62, 207, 142, 0.08)` (uses `--signal`, not `--coord` — this is session health context, not coordination)
- Border-bottom: `1px solid rgba(62, 207, 142, 0.20)`
- Radius: 5px 5px 0 0
- Font: 10px, `--signal` color
- Suppressed when all sessions share a single cwd (no distinguishing info)

---

## Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-04-25 | JetBrains Mono everywhere | Terminal-layer aesthetic; infrastructure, not app; users are terminal-native |
| 2026-04-25 | Blue (#3b82f6) for coordination, green for session health | Strict semantic split; makes coordination layer visually distinct from session health |
| 2026-04-25 | Visible grid background | Reinforces "coordinate space" mental model; screenshot-identifiable; no other tool in this space does it |
| 2026-04-25 | Card radius 5px (was 8px) | Data panel feel, not consumer app card |
| 2026-04-25 | Card padding 12px (was 16px) | Compact density appropriate for monitoring tools |
| 2026-04-25 | Pipe edges as hero visual | Defining identity for humOS; persistent edges (not just fire animation) make coordination always visible |
| 2026-04-25 | Amber (#f59e0b) as waiting semantic color | Formalizes existing implied waiting color; completes the three-state session health vocabulary |
