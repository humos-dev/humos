# humOS

Coordination runtime for AI agents. Provides pipe/signal/join primitives for multi-agent orchestration.

**Strategy:** See `STRATEGY.md` for the scaling law thesis, latent demand framing, and positioning.

**Stack:** Rust (Tauri v2) + React + TypeScript  
**Dev:** `PATH="$HOME/.cargo/bin:$PATH" npm run tauri dev`  
**Repo:** https://github.com/BoluOgunbiyi/humos

## Design System

Always read `DESIGN.md` before making any visual or UI decisions.
All font choices, colors, spacing, radius, and aesthetic direction are defined there.
Do not deviate without explicit approval.

Key rules from DESIGN.md:
- Font: JetBrains Mono everywhere — no system font, no Inter for rendered text
- Green (#3ecf8e) = session health only. Blue (#3b82f6) = coordination (pipes/signals) only. Never swap.
- Card radius: 5px. Card padding: 12px. Do not use 8px radius or 16px padding.
- Grid background on app bg — CSS only, no JS
- Pipe edges: always visible (not just on fire), blue coord color, arrowhead on active connections

In QA mode, flag any code that doesn't match DESIGN.md.

## Skill routing

When the user's request matches an available skill, ALWAYS invoke it using the Skill
tool as your FIRST action. Do NOT answer directly, do NOT use other tools first.
The skill has specialized workflows that produce better results than ad-hoc answers.

Key routing rules:
- Product ideas, "is this worth building", brainstorming → invoke office-hours
- Bugs, errors, "why is this broken", 500 errors → invoke investigate
- Ship, deploy, push, create PR → invoke ship
- QA, test the site, find bugs → invoke qa
- Code review, check my diff → invoke review
- Update docs after shipping → invoke document-release
- Weekly retro → invoke retro
- Design system, brand → invoke design-consultation
- Visual audit, design polish → invoke design-review
- Architecture review → invoke plan-eng-review
- Save progress, checkpoint, resume → invoke checkpoint
- Code quality, health check → invoke health