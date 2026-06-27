use crate::state::AppState;
use look_engine::config::RuntimeConfig;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

#[derive(Serialize)]
pub struct SearchResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub path: String,
    pub score: i64,
}

#[derive(Serialize)]
pub struct SearchPayload {
    pub count: usize,
    pub results: Vec<SearchResult>,
}

#[derive(Serialize)]
pub struct UsageResult {
    pub ok: bool,
    pub error: Option<String>,
}

const DEFAULT_SEARCH_LIMIT: u32 = 40;
const MAX_SEARCH_LIMIT: u32 = 100;

#[tauri::command]
pub fn search(state: State<'_, AppState>, query: String, limit: u32) -> SearchPayload {
    let max = if limit == 0 {
        DEFAULT_SEARCH_LIMIT
    } else {
        limit.min(MAX_SEARCH_LIMIT)
    } as usize;

    let scored = state.with_engine(|engine| engine.search_scored(&query, max));

    let results: Vec<SearchResult> = scored
        .into_iter()
        .map(|(candidate, score)| SearchResult {
            id: candidate.id.to_string(),
            kind: candidate.kind.as_str().to_string(),
            title: candidate.title.to_string(),
            subtitle: candidate.subtitle.as_deref().map(str::to_string),
            path: candidate.path.to_string(),
            score,
        })
        .collect();

    SearchPayload {
        count: results.len(),
        results,
    }
}

#[tauri::command]
pub fn record_usage(
    state: State<'_, AppState>,
    candidate_id: String,
    action: String,
) -> UsageResult {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let valid_actions = ["open_app", "open_file", "open_folder"];
    if !valid_actions.contains(&action.as_str()) {
        return UsageResult {
            ok: false,
            error: Some(format!("Invalid action: {action}")),
        };
    }

    let found = state.with_engine_mut(|engine| engine.record_usage_in_memory(&candidate_id, now));

    if found {
        let db_path = crate::state::default_db_path();
        if let Ok(store) = look_storage::SqliteStore::open(&db_path) {
            let _ = store.record_usage_event(&candidate_id, &action);
        }
    }

    UsageResult {
        ok: found,
        error: if found {
            None
        } else {
            Some(format!("Candidate not found: {candidate_id}"))
        },
    }
}

#[tauri::command]
pub fn open_path(
    window: tauri::WebviewWindow,
    path: String,
    kind: Option<String>,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] id: Option<String>,
) -> Result<(), String> {
    // Windows classic applets: look-cmd://program[?args].
    // - `program` alone (e.g. "devmgmt.msc", "appwiz.cpl", "regedit.exe") →
    //   open::that → ShellExecuteW, which does file-association lookup. This is
    //   required for .msc / .cpl because CreateProcessW (what Command::new
    //   uses) won't launch non-executable data files directly.
    // - `program?args` (e.g. rundll32.exe with a DLL+entry) → Command::new,
    //   because ShellExecute can't argv-parse a rundll32 command line.
    #[cfg(target_os = "windows")]
    if let Some(rest) = path.strip_prefix("look-cmd://") {
        let _ = window.hide();
        match rest.split_once('?') {
            Some((program, args)) => {
                let program = program.to_string();
                let args = args.to_string();
                std::thread::spawn(move || {
                    if let Err(e) = std::process::Command::new(&program)
                        .arg(&args)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        eprintln!("[open_path] look-cmd spawn {program:?} failed: {e}");
                    }
                });
            }
            None => {
                let program = rest.to_string();
                std::thread::spawn(move || {
                    if let Err(e) = open::that(&program) {
                        eprintln!("[open_path] look-cmd open {program:?} failed: {e}");
                    }
                });
            }
        }
        return Ok(());
    }

    // Linux system settings: settings://panel → gnome-control-center panel
    #[cfg(target_os = "linux")]
    if let Some(panel) = path.strip_prefix("settings://") {
        let _ = window.hide();
        let panel = panel.to_string();
        std::thread::spawn(move || {
            // D-Bus activation: works on GNOME, properly focuses the window.
            let dbus_ok = std::process::Command::new("gdbus")
                .args([
                    "call",
                    "--session",
                    "--dest",
                    "org.gnome.Settings",
                    "--object-path",
                    "/org/gnome/Settings",
                    "--method",
                    "org.freedesktop.Application.ActivateAction",
                    "launch-panel",
                    &format!("[<'{panel}'>, <@av []>]"),
                    "{}",
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            // Fallback: direct command (KDE, non-GNOME desktops)
            if !dbus_ok {
                let _ = std::process::Command::new("gnome-control-center")
                    .arg(&panel)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        });
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        // An "app" path is a URL only when its FIRST token carries the scheme
        // (e.g. `https://example.com`). Desktop Exec strings like
        // `steam steam://run/570` have the `://` embedded in an argument and
        // must still go through launch_app, otherwise we hand the whole
        // command line to xdg-open and nothing happens.
        let path_is_url = path
            .split_whitespace()
            .next()
            .is_some_and(|tok| tok.contains("://"));
        eprintln!("[open_path] path={path:?} kind={kind:?} id={id:?} path_is_url={path_is_url}");
        if kind.as_deref() == Some("app") && !path_is_url {
            let result = launch_app(&path, id.as_deref());
            if result.is_ok() {
                let _ = window.hide();
            }
            return result;
        }
    }

    if kind.as_deref() == Some("browser") {
        let _ = window.hide();
        std::thread::spawn(move || {
            let _ = open::that(&path);
            // Give the browser a beat to receive the URL and surface its new
            // tab — otherwise the focus attempt below races the spawn and
            // sees no matching window yet.
            #[cfg(target_os = "linux")]
            {
                std::thread::sleep(std::time::Duration::from_millis(BROWSER_FOCUS_DELAY_MS));
                focus_default_browser();
            }
        });
        Ok(())
    } else {
        // Windows: before launching a fresh instance, try to raise an existing
        // window for the same .exe / .lnk / UWP AUMID. Must run while Look
        // still holds foreground — SetForegroundWindow fails after hide().
        #[cfg(target_os = "windows")]
        if kind.as_deref() == Some("app")
            && crate::platform::windows::window_focus::try_focus_existing(&path)
        {
            let _ = window.hide();
            return Ok(());
        }

        let _ = window.hide();
        std::thread::spawn(move || {
            #[cfg(target_os = "linux")]
            {
                // Clear LD_LIBRARY_PATH so child processes use system libraries.
                // On NixOS, the Tauri dev shell may set paths that conflict with
                // system apps (glibc version mismatch).
                let _ = std::process::Command::new("xdg-open")
                    .arg(&path)
                    .env_remove("LD_LIBRARY_PATH")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = open::that(&path);
            }
        });
        Ok(())
    }
}

#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    let path_ref = std::path::Path::new(&path);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .arg("/select,")
            .arg(path_ref)
            .spawn()
            .map_err(|e| format!("Failed to reveal: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        let dir = if path_ref.is_file() {
            path_ref
                .parent()
                .unwrap_or(path_ref)
                .to_string_lossy()
                .to_string()
        } else {
            path.clone()
        };
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to reveal: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
pub fn reload_config(state: State<'_, AppState>) -> bool {
    // The engine caches the parsed `~/.look.config` across calls (skips a disk
    // read on every refresh). When the user explicitly reloads, drop the cache
    // so the next bootstrap picks up their edits.
    RuntimeConfig::invalidate_cache();
    state.request_index_refresh()
}

#[tauri::command]
pub fn request_index_refresh(state: State<'_, AppState>) -> bool {
    state.request_index_refresh()
}

#[tauri::command]
pub fn force_index_refresh(state: State<'_, AppState>) -> bool {
    state.force_index_refresh()
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    eprintln!("look: quit via Alt+Shift+Q");
    app.exit(0);
}

#[tauri::command]
pub fn toggle_window(window: tauri::WebviewWindow) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}

// --- App launching helpers ---

/// Build a Command that runs `prog` inside the user's systemd session, so
/// the child sees the user manager's environment (current XAUTHORITY for
/// XWayland, DBUS_SESSION_BUS_ADDRESS, etc.) rather than whatever Look
/// inherited. Falls back to a plain spawn when systemd-run isn't available.
///
/// Without this, a dev-mode Look launched from a long-lived `nix develop` /
/// terminal shell carries the X11 cookie path it picked up at shell start —
/// stale after the next mutter/XWayland restart — so spawned GUI children
/// (firefox, etc.) fail with "cannot open display: :0" while gtk-launch
/// itself still reports success.
///
/// `KillMode=process` keeps the actual GUI app alive after gtk-launch / gio
/// launch (the unit's main process) exits.
#[cfg(target_os = "linux")]
fn user_session_cmd(prog: &str) -> std::process::Command {
    use std::sync::OnceLock;
    static SYSTEMD_RUN: OnceLock<bool> = OnceLock::new();
    let available = *SYSTEMD_RUN.get_or_init(|| {
        // Exercise the actual `--user` path (a no-op transient unit) — checking
        // only `systemd-run --version` succeeds on systems that have the binary
        // but no usable per-user manager (containers, minimal installs), and
        // would route every launch through a wrapper that then fails.
        std::process::Command::new("systemd-run")
            .args(["--user", "--quiet", "--wait", "--collect", "--", "true"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    });
    if available {
        let mut cmd = std::process::Command::new("systemd-run");
        cmd.args([
            "--user",
            "--collect",
            "--quiet",
            "--property=KillMode=process",
            "--",
            prog,
        ]);
        cmd
    } else {
        std::process::Command::new(prog)
    }
}

#[cfg(target_os = "linux")]
fn launch_app(exec: &str, id: Option<&str>) -> Result<(), String> {
    let desktop_file = id
        .and_then(|id| id.strip_prefix("app:"))
        .and_then(find_desktop_file);

    // Try to focus an existing window before launching a new instance.
    if let Some(ref real_path) = desktop_file
        && try_focus_existing(real_path)
    {
        return Ok(());
    }

    // Build the launch chain: gtk-launch → gio launch → direct exec.
    // gtk-launch is preferred because gio launch uses D-Bus activation
    // which can silently fail to show a window on first invocation.
    // Use the resolved desktop_file path (case-preserving) rather than the
    // raw frontend ID — IDs may be lowercased upstream while gtk-launch is
    // case-sensitive ("org.gnome.Nautilus" works, "org.gnome.nautilus" does not).
    let desktop_path = desktop_file.clone();
    let desktop_name = desktop_file
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|f| f.to_str())
        .and_then(|f| f.strip_suffix(".desktop"))
        .map(String::from);
    let exec_cmd = exec.to_string();
    // Steam game shortcuts (Exec like `steam steam://run/<id>` or
    // `/usr/bin/steam steam://run/<id>`) need the Steam client up before the
    // URL is issued; on cold start Steam's bootstrap drops the URL silently
    // and nothing visible happens. Detect any Exec carrying a `steam://`
    // URL and, when Steam isn't running, pre-start the client and wait for
    // /proc + a short settle window so the launch chain below hands off to
    // a Steam that's ready to receive the URL.
    let exec_has_steam_url = exec_cmd.contains("steam://");
    let steam_already_running = crate::platform::linux::process::is_running("steam");
    let needs_steam_warmup = exec_has_steam_url && !steam_already_running;
    eprintln!(
        "[launch] exec={exec_cmd:?} desktop_name={desktop_name:?} \
         steam_url={exec_has_steam_url} steam_running={steam_already_running} \
         warmup={needs_steam_warmup}"
    );

    std::thread::spawn(move || {
        if needs_steam_warmup {
            eprintln!("[launch] Steam URL exec on cold start; pre-starting steam");
            let _ = user_session_cmd("steam")
                .env_remove("LD_LIBRARY_PATH")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            // Poll for the steam process (up to ~5s), then give the client a
            // moment for its IPC to come up before issuing the URL.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while std::time::Instant::now() < deadline {
                if crate::platform::linux::process::is_running("steam") {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }

        if let Some(ref name) = desktop_name {
            eprintln!("[launch] trying gtk-launch {name}");
            let result = user_session_cmd("gtk-launch")
                .arg(name)
                .env_remove("LD_LIBRARY_PATH")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .output();
            match &result {
                Ok(output) if output.status.success() => {
                    eprintln!("[launch] gtk-launch succeeded");
                    return;
                }
                Ok(output) => {
                    let err = String::from_utf8_lossy(&output.stderr);
                    eprintln!("[launch] gtk-launch failed (exit {}): {err}", output.status);
                }
                Err(e) => eprintln!("[launch] gtk-launch not found: {e}"),
            }
        }

        if let Some(ref real_path) = desktop_path {
            eprintln!("[launch] trying gio launch {real_path}");
            let result = user_session_cmd("gio")
                .args(["launch", real_path])
                .env_remove("LD_LIBRARY_PATH")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .output();
            match &result {
                Ok(output) if output.status.success() => {
                    eprintln!("[launch] gio launch succeeded");
                    return;
                }
                Ok(output) => {
                    let err = String::from_utf8_lossy(&output.stderr);
                    eprintln!("[launch] gio launch failed (exit {}): {err}", output.status);
                }
                Err(e) => eprintln!("[launch] gio not found: {e}"),
            }
        }

        let mut parts = exec_cmd.split_whitespace();
        if let Some(cmd) = parts.next() {
            let args: Vec<&str> = parts.filter(|s| !s.starts_with('%')).collect();
            eprintln!("[launch] trying direct exec: {cmd} {}", args.join(" "));
            match user_session_cmd(cmd)
                .args(&args)
                .env_remove("LD_LIBRARY_PATH")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(_) => eprintln!("[launch] direct exec spawned"),
                Err(e) => eprintln!("[launch] direct exec failed: {e}"),
            }
        }
    });

    Ok(())
}

/// Delay between handing the URL to xdg-open and trying to focus the
/// browser window — gives the browser time to receive the URL and surface
/// its new tab so the focus call below sees a matching window.
#[cfg(target_os = "linux")]
const BROWSER_FOCUS_DELAY_MS: u64 = 150;

/// Bring the user's default browser window to the foreground. Resolves the
/// browser via `xdg-mime query default x-scheme-handler/https` so we focus
/// the exact browser xdg-open just sent the URL to — not whichever browser
/// happened to come first in a hard-coded candidate list (which would
/// route the focus to the wrong window when the user has multiple browsers
/// open, e.g. Brave default but Firefox also running).
///
/// Tries the GNOME Shell extension first (FocusApp by .desktop id, works on
/// GNOME Wayland), then falls back to WM_CLASS / app_id matching via
/// try_focus_window which knows about Sway, i3, and X11.
#[cfg(target_os = "linux")]
fn focus_default_browser() -> bool {
    let Ok(output) = std::process::Command::new("xdg-mime")
        .args(["query", "default", "x-scheme-handler/https"])
        .output()
    else {
        return false;
    };
    let desktop_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if desktop_id.is_empty() {
        return false;
    }

    // GNOME Shell extension — exact desktop id.
    if crate::platform::linux::gnome_ext::try_focus_app(&desktop_id) {
        return true;
    }

    // Strip ".desktop" suffix to get the base id (e.g. "brave-browser").
    // That's typically the WM_CLASS / app_id the browser advertises — Sway
    // and i3 match it case-insensitively via the (?i) flag inside
    // try_focus_window, so "brave-browser" matches "Brave-browser" too.
    let base = desktop_id.strip_suffix(".desktop").unwrap_or(&desktop_id);
    if try_focus_window(base) {
        return true;
    }
    // Some browsers use the last path segment of a reverse-DNS id as their
    // class (e.g. "org.mozilla.firefox.desktop" → "firefox"). Try that too.
    if let Some(tail) = base.rsplit('.').next()
        && tail != base
        && try_focus_window(tail)
    {
        return true;
    }
    false
}

#[cfg(target_os = "linux")]
fn try_focus_window(wm_class: &str) -> bool {
    // Sway (Wayland): SWAYSOCK is set, swaymsg shares i3's IPC and CLI but
    // Wayland-native clients identify by `app_id`, not WM_CLASS. XWayland
    // clients still fall back to class/instance. The x11rb path below can't
    // see Wayland windows at all, so this branch is the only thing that
    // brings non-XWayland browsers (firefox-wayland, brave Wayland) forward.
    if std::env::var("SWAYSOCK").is_ok() {
        for criterion in [
            format!("[app_id=\"(?i){wm_class}\"] focus"),
            format!("[class=\"(?i){wm_class}\"] focus"),
            format!("[instance=\"(?i){wm_class}\"] focus"),
        ] {
            if let Ok(output) = std::process::Command::new("swaymsg")
                .arg(&criterion)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("\"success\":true") {
                    return true;
                }
            }
        }
        return false;
    }

    // i3 window manager — use i3-msg exclusively (i3 ignores raw X11
    // _NET_ACTIVE_WINDOW messages, so the x11rb fallback would report
    // success without actually focusing).  Try both class and instance
    // criteria: GTK apps often set instance to the reverse-DNS app ID
    // (e.g. "org.pwmt.zathura") while class is the short name ("Zathura").
    if std::env::var("I3SOCK").is_ok() {
        for criterion in [
            format!("[class=\"(?i){wm_class}\"] focus"),
            format!("[instance=\"(?i){wm_class}\"] focus"),
        ] {
            if let Ok(output) = std::process::Command::new("i3-msg")
                .arg(&criterion)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("\"success\":true") {
                    return true;
                }
            }
        }
        return false;
    }

    // Non-i3: try i3-msg anyway (might be running), then x11rb fallback.
    if let Ok(output) = std::process::Command::new("i3-msg")
        .arg(format!("[class=\"(?i){wm_class}\"] focus"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("\"success\":true") {
            return true;
        }
    }

    // Linux: x11rb _NET_ACTIVE_WINDOW (covers GNOME, KDE, etc.)
    #[cfg(target_os = "linux")]
    if crate::platform::linux::window_focus::try_focus(wm_class) {
        return true;
    }

    false
}

/// Public wrapper for process::activate_running_app.
#[cfg(target_os = "linux")]
pub fn try_focus_existing_pub(desktop_path: &str) -> bool {
    try_focus_existing(desktop_path)
}

/// Public wrapper for process::activate_running_app.
#[cfg(target_os = "linux")]
pub fn try_focus_window_pub(wm_class: &str) -> bool {
    try_focus_window(wm_class)
}

/// Try to focus an existing window for a desktop file.
/// Dispatches to the appropriate method based on display server / compositor.
#[cfg(target_os = "linux")]
fn try_focus_existing(desktop_path: &str) -> bool {
    let wm_class = parse_desktop_field(desktop_path, "StartupWMClass");
    let stem = std::path::Path::new(desktop_path)
        .file_stem()
        .and_then(|f| f.to_str())
        .map(String::from);

    // For reverse-DNS stems like "org.pwmt.zathura", also try the last
    // segment ("zathura") — many apps use the short name as WM_CLASS even
    // when the desktop file uses the full reverse-DNS ID.
    let short_name = stem.as_deref().and_then(|s| {
        if s.contains('.') {
            s.rsplit('.').next().map(String::from)
        } else {
            None
        }
    });

    let mut candidates: Vec<&str> = [wm_class.as_deref(), stem.as_deref(), short_name.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    candidates.dedup();
    eprintln!("[focus] try_focus_existing desktop={desktop_path} candidates={candidates:?}");

    #[cfg(target_os = "linux")]
    if crate::platform::linux::transparency::is_wayland() {
        return try_focus_wayland(desktop_path, &candidates);
    }

    for id in &candidates {
        if try_focus_window(id) {
            return true;
        }
    }
    false
}

/// Wayland focus: dispatch to the active compositor's IPC.
#[cfg(target_os = "linux")]
fn try_focus_wayland(desktop_path: &str, candidates: &[&str]) -> bool {
    if crate::platform::linux::wm::is_sway() {
        return candidates.iter().any(|id| try_focus_sway(id));
    }
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return candidates.iter().any(|id| try_focus_hyprland(id));
    }
    // GNOME Wayland: use GNOME Shell extension
    let desktop_id = std::path::Path::new(desktop_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    !desktop_id.is_empty() && crate::platform::linux::gnome_ext::try_focus_app(desktop_id)
}

#[cfg(target_os = "linux")]
fn try_focus_sway(app_id: &str) -> bool {
    // Try the native wlr-foreign-toplevel protocol first (works for any
    // wlroots compositor); fall back to sway IPC if the protocol isn't
    // available.
    if crate::platform::linux::wlr_focus::try_focus(app_id) {
        return true;
    }
    for criteria in [
        format!("[app_id=\"(?i){app_id}\"] focus"),
        format!("[class=\"(?i){app_id}\"] focus"),
    ] {
        if let Ok(output) = std::process::Command::new("swaymsg")
            .arg(&criteria)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("\"success\": true") {
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn try_focus_hyprland(class: &str) -> bool {
    eprintln!("[focus] hyprland try class={class}");
    // Primary path: native wlr-foreign-toplevel-management. Works regardless
    // of the broken hyprctl dispatcher on v0.55+.
    if crate::platform::linux::wlr_focus::try_focus(class) {
        eprintln!("[focus] hyprland focus via wlr-foreign-toplevel succeeded");
        return true;
    }
    // Fallback for Hyprland < v0.55 where the legacy dispatcher still works
    // (and the wlr protocol may not be advertised).
    if !hyprland_has_client(class) {
        return false;
    }
    let _ = std::process::Command::new("hyprctl")
        .args(["dispatch", "focuswindow", &format!("class:{class}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    if hyprland_active_class_matches(class) {
        eprintln!("[focus] hyprland legacy dispatcher worked");
        return true;
    }
    eprintln!("[focus] hyprland focus failed for class={class}, falling through to launch chain");
    false
}

#[cfg(target_os = "linux")]
fn hyprland_has_client(class: &str) -> bool {
    let Ok(output) = std::process::Command::new("hyprctl")
        .args(["clients", "-j"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    json_has_class(&String::from_utf8_lossy(&output.stdout), class)
}

#[cfg(target_os = "linux")]
fn hyprland_active_class_matches(class: &str) -> bool {
    let Ok(output) = std::process::Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    json_has_class(&String::from_utf8_lossy(&output.stdout), class)
}

#[cfg(target_os = "linux")]
fn json_has_class(json: &str, class: &str) -> bool {
    let json = json.to_lowercase();
    let needle = class.to_lowercase();
    for key in ["\"class\":", "\"initialclass\":"] {
        let mut rest = json.as_str();
        while let Some(idx) = rest.find(key) {
            rest = &rest[idx + key.len()..];
            let trimmed = rest.trim_start();
            if let Some(after_quote) = trimmed.strip_prefix('"')
                && let Some(end) = after_quote.find('"')
                && after_quote[..end] == needle
            {
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn parse_desktop_field(path: &str, field: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let prefix = format!("{field}=");
    let mut in_desktop_entry = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some(val) = line.strip_prefix(&prefix) {
            let val = val.trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn find_desktop_file(id_path: &str) -> Option<String> {
    if std::path::Path::new(id_path).exists() {
        return Some(id_path.to_string());
    }
    let path = std::path::Path::new(id_path);
    let dir = path.parent()?;
    let filename_lower = path.file_name()?.to_str()?.to_lowercase();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        if entry.file_name().to_str()?.to_lowercase() == filename_lower {
            return Some(entry.path().to_string_lossy().to_string());
        }
    }
    None
}
