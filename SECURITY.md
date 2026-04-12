# Security Policy

## Reporting a vulnerability

Email security concerns to **security@humos.dev** (or open a private GitHub advisory if you prefer).

We will acknowledge within 48 hours and aim to fix critical issues within 7 days.

## Scope

humOS runs entirely on your local machine. There are no network services, no cloud backend, no API keys stored. The primary security surface is:

- **AppleScript injection** via `inject_message` (message content passes through `do script` / `write text`)
- **Clipboard handling** during terminal injection
- **JSONL parsing** of untrusted session files in `~/.claude/projects/`
- **Tauri IPC** boundary between the Rust backend and the React frontend

## Out of scope

- Vulnerabilities in Claude CLI itself (report to Anthropic)
- macOS system-level issues (report to Apple)
- Denial of service via large JSONL files (known limitation, not a security issue)
