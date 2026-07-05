const TIMEOUT_SECS: u64 = 10;
const MAX_OUTPUT_BYTES: usize = 800;
const POLL_INTERVAL_MS: u64 = 50;

#[tauri::command]
pub fn run_shell_command(cmd: String) -> Result<String, String> {
    // sh on Unix, cmd /C on Windows. The frontend doesn't know which it's on
    // and lets the user type whatever fits their muscle memory.
    #[cfg(target_os = "windows")]
    let spawned = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", &cmd])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .creation_flags(crate::consts::CREATE_NO_WINDOW)
            .spawn()
    };
    #[cfg(not(target_os = "windows"))]
    let spawned = crate::platform::linux::host_command("sh")
        .args(["-c", &cmd])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = spawned.map_err(|e| format!("Failed to run: {e}"))?;

    let timeout = std::time::Duration::from_secs(TIMEOUT_SECS);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(format!("(timed out after {TIMEOUT_SECS}s)"));
                }
                std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(e) => return Err(format!("Wait error: {e}")),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Output error: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut result = String::new();
    if !stdout.is_empty() {
        result.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&stderr);
    }

    if result.len() > MAX_OUTPUT_BYTES {
        result.truncate(MAX_OUTPUT_BYTES);
        result.push_str("\n... (truncated)");
    }

    if result.is_empty() {
        result = format!("(exit code: {})", output.status.code().unwrap_or(-1));
    }

    Ok(result)
}
