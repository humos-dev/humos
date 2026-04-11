# Changelog

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
