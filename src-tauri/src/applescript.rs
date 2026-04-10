use std::process::Command;

/// Focus the Terminal window whose working directory matches `cwd`.
/// Uses AppleScript to enumerate Terminal tabs and activate the matching one.
/// Falls back to focusing any Terminal window if no exact match is found.
pub fn focus_terminal(cwd: &str) -> Result<(), String> {
    // Escape the cwd for safe embedding in AppleScript string
    let cwd_escaped = cwd.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        r#"
tell application "Terminal"
    set targetCwd to "{cwd}"
    set found to false
    repeat with w in windows
        repeat with t in tabs of w
            try
                set tabCwd to custom title of t
            on error
                set tabCwd to ""
            end try
            if tabCwd contains targetCwd then
                set current settings of t to current settings of t
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
        cwd = cwd_escaped
    );

    run_applescript(&script)
}

/// Focus the matching Terminal window and inject a message by writing to
/// clipboard then simulating Cmd+V and Return.
pub fn inject_message(cwd: &str, message: &str) -> Result<(), String> {
    // First focus the terminal
    focus_terminal(cwd)?;

    // Escape message for AppleScript
    let msg_escaped = message.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        r#"
set the clipboard to "{message}"
tell application "System Events"
    tell process "Terminal"
        keystroke "v" using command down
        key code 36
    end tell
end tell
"#,
        message = msg_escaped
    );

    run_applescript(&script)
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
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("osascript error: {}", stderr))
    }
}
