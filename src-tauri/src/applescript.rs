use std::process::Command;

/// Focus the Terminal window whose working directory matches `cwd`.
/// Uses AppleScript to enumerate Terminal tabs and activate the matching one.
/// Falls back to focusing any Terminal window if no exact match is found.
pub fn focus_terminal(cwd: &str) -> Result<(), String> {
    let cwd_escaped = escape_applescript(cwd);
    let last_segment_raw = cwd.split('/').filter(|s| !s.is_empty()).last().unwrap_or(cwd);
    let last_segment = escape_applescript(last_segment_raw);

    let script = format!(
        r#"
tell application "Terminal"
    set targetCwd to "{cwd}"
    set targetName to "{last_segment}"
    set found to false
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

/// Find the Terminal tab whose working directory matches `cwd` and inject
/// a message using `do script "{message}" in t`.
///
/// This sends the message text directly to the terminal's input buffer.
/// When Claude CLI is running interactively and waiting for input, the text
/// arrives at its stdin — exactly the injection mechanism needed for pipes
/// and the Send button.
///
/// The message is escaped for AppleScript string embedding (backslash, double
/// quotes, and single quotes). Pipe messages are constructed by humOS's own
/// `build_message()` — they don't include untrusted shell content.
pub fn inject_message(cwd: &str, message: &str) -> Result<(), String> {
    let cwd_escaped = escape_applescript(cwd);
    let last_segment_raw = cwd.split('/').filter(|s| !s.is_empty()).last().unwrap_or(cwd);
    let last_segment = escape_applescript(last_segment_raw);
    let msg_escaped = escape_applescript(message);

    let script = format!(
        r#"
tell application "Terminal"
    set targetCwd to "{cwd}"
    set targetName to "{last_segment}"
    set injected to false
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

/// Escape a string for safe embedding inside an AppleScript double-quoted string.
/// Also replaces straight single quotes with the typographic right single quote (')
/// so they don't interfere with shell interpretation when `do script` executes.
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\'', "\u{2019}") // ' → ' (right single quotation mark)
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
        return "Terminal.app can't find this session's window. Is it still open? \
                Check System Settings → Privacy & Security → Accessibility if Terminal is listed."
            .to_string();
    }
    if raw.contains("(-1712)") {
        return "Terminal.app timed out. It may be busy or unresponsive.".to_string();
    }
    if raw.contains("(-1743)") || raw.contains("not allowed assistive") {
        return "humOS needs Accessibility permission to control Terminal. \
                Open System Settings → Privacy & Security → Accessibility and enable humOS."
            .to_string();
    }
    if raw.contains("1001") {
        // Our own error number for "no tab found"
        return "No Terminal window found for this session's directory. \
                Make sure Terminal is open with the session's working directory."
            .to_string();
    }
    format!("AppleScript error: {}. Check Terminal.app permissions in System Settings.", raw.trim())
}
