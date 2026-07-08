//! Last-resort crash reporting for failures the health notice can't reach.
//!
//! The health banner needs a live webview; a panic during startup (WebView2
//! missing on Windows, plugin init, the setup hook) kills the process before
//! one exists, and a menu-launched Look dies as an invisible no-show. The
//! panic hook writes the panic to a crash log and, for main-thread panics,
//! shows a native dialog that doesn't depend on the webview.
//!
//! Out of scope: segfaults in native code (WebKitGTK, GPU drivers) never
//! reach a Rust panic hook, and loader failures (missing shared libraries)
//! kill the process before any of this runs.

use std::io::Write;
use std::path::{Path, PathBuf};

const CRASH_LOG_NAME: &str = "crash.log";
const DIALOG_TITLE: &str = "Look";

/// Install the panic hook. Must run first in `main` so even Tauri
/// builder/setup failures are covered.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Keep the default stderr/backtrace output for terminal launches.
        default_hook(info);

        let message = panic_message(info);
        let log_path = append_crash_log(&message);

        // Background-thread panics don't take the process down - stay quiet
        // beyond the log. A main-thread panic unwinds out of `main`, so this
        // dialog is the only thing the user would ever see.
        if std::thread::current().name() == Some("main") {
            show_native_dialog(&message, log_path.as_deref());
        }
    }));
}

fn panic_message(info: &std::panic::PanicHookInfo) -> String {
    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic");
    match info.location() {
        Some(loc) => format!("{payload} (at {loc})"),
        None => payload.to_string(),
    }
}

/// Append the crash to `<state dir>/look/crash.log` and return its path.
/// Linux: ~/.local/state/look (XDG state dir). Windows has no state-dir
/// notion - falls back to %LOCALAPPDATA%\look, next to the index database.
fn append_crash_log(message: &str) -> Option<PathBuf> {
    let dir = dirs::state_dir()
        .or_else(dirs::data_local_dir)?
        .join("look");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(CRASH_LOG_NAME);
    let unix_s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()?;
    writeln!(
        file,
        "[{unix_s}] lookapp {}: {message}",
        env!("APP_VERSION")
    )
    .ok()?;
    Some(path)
}

fn show_native_dialog(message: &str, log_path: Option<&Path>) {
    let text = match log_path {
        Some(path) => format!(
            "Look crashed: {message}\n\nDetails were written to:\n{}",
            path.display()
        ),
        None => format!("Look crashed: {message}"),
    };
    native_error_dialog(&text);
}

#[cfg(target_os = "windows")]
fn native_error_dialog(text: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW};
    use windows::core::PCWSTR;

    let to_wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let text_w = to_wide(text);
    let title_w = to_wide(DIALOG_TITLE);
    // MessageBoxW only needs user32.dll, so it works even when the crash is
    // WebView2 itself failing to initialize.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_ICONERROR,
        );
    }
}

#[cfg(target_os = "linux")]
fn native_error_dialog(text: &str) {
    use crate::platform::linux::host_command;

    // zenity covers GNOME, kdialog KDE, notify-send most other desktops;
    // xmessage is the bare-X11 last resort. First tool that reports success
    // wins. host_command keeps the AppImage's LD_LIBRARY_PATH out of them.
    let attempts: [(&str, Vec<String>); 4] = [
        (
            "zenity",
            vec![
                "--error".into(),
                format!("--title={DIALOG_TITLE}"),
                format!("--text={text}"),
            ],
        ),
        (
            "kdialog",
            vec![
                "--title".into(),
                DIALOG_TITLE.into(),
                "--error".into(),
                text.into(),
            ],
        ),
        (
            "notify-send",
            vec![
                "-u".into(),
                "critical".into(),
                DIALOG_TITLE.into(),
                text.into(),
            ],
        ),
        ("xmessage", vec!["-center".into(), text.into()]),
    ];
    for (cmd, args) in attempts {
        let shown = host_command(cmd)
            .args(&args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if shown {
            return;
        }
    }
}
