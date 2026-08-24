#[cfg(target_os = "linux")]
use crate::platform::linux::{host_command, user_session_command, user_session_command_for_status};
use crate::state::AppState;
use look_engine::config::RuntimeConfig;
use serde::Serialize;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, State};

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
        hide_armed(&window);
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
        hide_armed(&window);
        let panel = panel.to_string();
        std::thread::spawn(move || {
            // D-Bus activation: works on GNOME, properly focuses the window.
            let dbus_ok = host_command("gdbus")
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
                let _ = host_command("gnome-control-center")
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
                hide_armed(&window);
            }
            return result;
        }
    }

    if kind.as_deref() == Some("browser") {
        hide_armed(&window);
        std::thread::spawn(move || {
            // Not open::that on Linux: it spawns xdg-open with the inherited
            // env that host_command exists to scrub.
            #[cfg(target_os = "linux")]
            {
                let _ = host_command("xdg-open")
                    .arg(&path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                // Give the browser a beat to receive the URL and surface its
                // new tab; the focus attempt below races the spawn otherwise.
                std::thread::sleep(std::time::Duration::from_millis(
                    crate::consts::HANDLER_FOCUS_DELAY_MS,
                ));
                focus_default_browser();
            }
            #[cfg(not(target_os = "linux"))]
            let _ = open::that(&path);
        });
        Ok(())
    } else {
        // Windows: before launching a fresh instance, try to raise an existing
        // window for the same .exe / .lnk / UWP AUMID. Must run while Look
        // still holds foreground - SetForegroundWindow fails after hide().
        #[cfg(target_os = "windows")]
        if kind.as_deref() == Some("app")
            && crate::platform::windows::window_focus::try_focus_existing(&path)
        {
            hide_armed(&window);
            return Ok(());
        }

        // Shell namespace locations (e.g. `shell:RecycleBinFolder`) aren't
        // filesystem paths - ShellExecute can't always resolve them, but
        // Explorer opens them directly.
        #[cfg(target_os = "windows")]
        if path.starts_with("shell:") {
            hide_armed(&window);
            let _ = std::process::Command::new("explorer.exe")
                .arg(&path)
                .spawn();
            return Ok(());
        }

        hide_armed(&window);
        std::thread::spawn(move || {
            #[cfg(target_os = "linux")]
            {
                let _ = host_command("xdg-open")
                    .arg(&path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                // Same focus dance as the browser branch - Sway/i3 don't raise
                // the handler on xdg-open activation. Resolves the handler via
                // the file's MIME type so a PNG opened in Brave focuses Brave,
                // a PDF opened in Zathura focuses Zathura, etc.
                std::thread::sleep(std::time::Duration::from_millis(
                    crate::consts::HANDLER_FOCUS_DELAY_MS,
                ));
                focus_file_handler(&path);
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = open::that(&path);
            }
        });
        Ok(())
    }
}

/// Windows: `runas` launch. Async because the UAC prompt is modal and a sync
/// command would block the main thread; resolves only once the launch is
/// confirmed, so usage is never recorded for a declined prompt.
#[tauri::command]
pub async fn open_elevated(
    window: tauri::WebviewWindow,
    #[cfg_attr(not(target_os = "windows"), allow(unused_variables))] path: String,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        hide_armed(&window);
        let result = tauri::async_runtime::spawn_blocking(move || {
            let (program, args) = crate::platform::windows::launch::split_target(&path);
            crate::platform::windows::launch::run_as_admin(program, args)
        })
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
        if let Err(e) = &result {
            // Declined, refused, or the task died: don't leave Look hidden.
            eprintln!("[open_elevated] {e}");
            show_launcher(&window);
            let _ = window.set_focus();
        }
        result
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        Err("elevated launch is Windows only".into())
    }
}

/// Ctrl+F. The same reveal the `reveal` tool action falls back to when no
/// `file_manager` is declared, so both spellings select the file rather than
/// one of them merely opening its folder.
#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let outcome = crate::platform::linux::tools::reveal(&path);
    #[cfg(target_os = "windows")]
    let outcome = crate::platform::windows::tools::reveal(&path);

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let outcome: Result<(), String> = {
        let _ = path;
        Err("reveal is not supported on this platform".to_string())
    };

    outcome.map_err(|e| format!("Failed to reveal: {e}"))
}

#[tauri::command]
pub fn reload_config(state: State<'_, AppState>) -> bool {
    // The engine caches the parsed `~/.look/config` across calls (skips a disk
    // read on every refresh). When the user explicitly reloads, drop the cache
    // so the next bootstrap picks up their edits.
    RuntimeConfig::invalidate_cache();
    crate::clipboard::reload_from_config();
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
pub fn get_install_method() -> String {
    #[cfg(target_os = "windows")]
    {
        return crate::platform::windows::update::detect_install_method()
            .as_str()
            .to_string();
    }

    #[cfg(not(target_os = "windows"))]
    {
        "unknown".to_string()
    }
}

#[tauri::command]
pub fn start_windows_update(app: tauri::AppHandle, version: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        return crate::platform::windows::update::start(app, &version);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        let _ = version;
        Err("Windows self-update is unsupported on this platform".into())
    }
}

/// Longest the window stays up waiting for the frontend to paint the armed
/// frame. `confirm_hide` ends the wait as soon as that frame lands, so this
/// only runs out for a webview that never answers.
const HIDE_ARM_GRACE: std::time::Duration = std::time::Duration::from_millis(60);

/// Id of the dismissal still in flight, 0 when none is; a show clears it so a
/// fallback the user already undid can't pull the window back down.
static PENDING_HIDE: AtomicU64 = AtomicU64::new(0);
static HIDE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Arm the entrance, then hide once the frontend has painted that frame.
///
/// The compositor keeps the last buffer the webview painted and presents it
/// when the window maps again, so hiding in the same frame leaves the fully
/// revealed panel to flash on the next summon before the entrance rewinds and
/// replays. Every dismiss goes through here.
pub fn hide_armed(window: &tauri::WebviewWindow) {
    let arm = HIDE_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    PENDING_HIDE.store(arm, Ordering::Relaxed);
    let _ = window.emit(crate::consts::EVENT_WINDOW_HIDDEN, arm);
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(HIDE_ARM_GRACE).await;
        if PENDING_HIDE
            .compare_exchange(arm, 0, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            let _ = window.hide();
        }
    });
}

/// The frontend has painted the armed frame; the window can go now. Keyed on
/// `arm` so a late confirmation can't hide a window a later dismiss just armed.
/// `NonZeroU64` keeps the idle sentinel out of the compare: a payload of 0 is
/// rejected while deserializing, never as a match against an idle `PENDING_HIDE`.
#[tauri::command]
pub fn confirm_hide(window: tauri::WebviewWindow, arm: NonZeroU64) {
    if PENDING_HIDE
        .compare_exchange(arm.get(), 0, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        let _ = window.hide();
    }
}

/// Drop any dismissal in flight, show, and keep niri from tiling the window.
/// Every show goes through here.
pub fn show_launcher(window: &tauri::WebviewWindow) {
    PENDING_HIDE.store(0, Ordering::Relaxed);
    let _ = window.show();
    #[cfg(target_os = "linux")]
    if crate::platform::linux::wm::is_niri() {
        crate::platform::linux::niri::ensure_self_floating();
    }
}

#[tauri::command]
pub fn toggle_window(window: tauri::WebviewWindow) {
    if window.is_visible().unwrap_or(false) {
        hide_armed(&window);
    } else {
        show_launcher(&window);
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn hide_window(window: tauri::WebviewWindow) {
    hide_armed(&window);
}

/// Blur behind the surfaces the frontend paints, in logical pixels. A no-op
/// wherever the compositor has no such request (see platform::blur).
#[tauri::command]
pub fn set_blur_region(
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] window: tauri::WebviewWindow,
    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))] rects: Vec<
        crate::platform::BlurRect,
    >,
) {
    #[cfg(target_os = "linux")]
    if let Some(wid) = crate::platform::linux::window_focus::self_window() {
        let scale = window.scale_factor().unwrap_or(1.0);
        crate::platform::linux::blur::set_region(wid, &rects, scale);
    }
}

// --- App launching helpers ---

/// Run one rung of the launch chain and say whether it started the app.
///
/// Under the session wrapper the tool's own stderr goes to the journal, so the
/// exit status is the part that always carries; a message is printed only when
/// there is one.
#[cfg(target_os = "linux")]
fn launch_step(step: &str, command: &mut std::process::Command) -> bool {
    let result = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();

    match result {
        Ok(output) if output.status.success() => {
            eprintln!("[launch] {step} succeeded");
            true
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            match err.trim() {
                "" => eprintln!("[launch] {step} failed (exit {})", output.status),
                detail => eprintln!("[launch] {step} failed (exit {}): {detail}", output.status),
            }
            false
        }
        Err(e) => {
            eprintln!("[launch] {step} not available: {e}");
            false
        }
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
    // raw frontend ID - IDs may be lowercased upstream while gtk-launch is
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
            let _ = user_session_command("steam")
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
            if launch_step(
                "gtk-launch",
                user_session_command_for_status("gtk-launch").arg(name),
            ) {
                return;
            }
        }

        if let Some(ref real_path) = desktop_path {
            eprintln!("[launch] trying gio launch {real_path}");
            if launch_step(
                "gio launch",
                user_session_command_for_status("gio").args(["launch", real_path]),
            ) {
                return;
            }
        }

        let mut parts = exec_cmd.split_whitespace();
        if let Some(cmd) = parts.next() {
            let args: Vec<&str> = parts.filter(|s| !s.starts_with('%')).collect();
            eprintln!("[launch] trying direct exec: {cmd} {}", args.join(" "));
            match user_session_command(cmd)
                .args(&args)
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

/// Focus the window of a handler identified by its .desktop id. Tries the
/// GNOME Shell extension first (works on GNOME Wayland), then falls back to
/// WM_CLASS / app_id matching via try_focus_window which knows about Sway,
/// i3, and X11. Used by both the browser-URL path and the file-open path so
/// e.g. a PNG that routes to Brave focuses Brave on Sway, where xdg-open
/// itself doesn't raise the window.
#[cfg(target_os = "linux")]
fn focus_handler_by_desktop_id(desktop_id: &str) -> bool {
    if desktop_id.is_empty() {
        return false;
    }

    if crate::platform::linux::gnome_ext::try_focus_app(desktop_id) {
        return true;
    }

    // Strip ".desktop" suffix to get the base id (e.g. "brave-browser").
    // That's typically the WM_CLASS / app_id the app advertises - Sway and i3
    // match it case-insensitively via the (?i) flag inside try_focus_window,
    // so "brave-browser" matches "Brave-browser" too.
    let base = desktop_id.strip_suffix(".desktop").unwrap_or(desktop_id);
    if try_focus_window(base) {
        return true;
    }
    // Some apps use the last path segment of a reverse-DNS id as their class
    // (e.g. "org.mozilla.firefox" → "firefox").
    if let Some(tail) = base.rsplit('.').next()
        && tail != base
        && try_focus_window(tail)
    {
        return true;
    }
    false
}

/// Look up the default handler for a MIME type via xdg-mime. Returns the
/// raw .desktop id (e.g. "brave-browser.desktop") or None if unset.
#[cfg(target_os = "linux")]
fn default_handler_for_mime(mime: &str) -> Option<String> {
    let output = host_command("xdg-mime")
        .args(["query", "default", mime])
        .output()
        .ok()?;
    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!id.is_empty()).then_some(id)
}

/// Bring the user's default browser to the foreground. Resolves the browser
/// via `xdg-mime query default x-scheme-handler/https` so we focus the exact
/// browser xdg-open just sent the URL to - not whichever browser happened to
/// come first in a hard-coded candidate list (which would route the focus to
/// the wrong window when the user has multiple browsers open, e.g. Brave
/// default but Firefox also running).
#[cfg(target_os = "linux")]
fn focus_default_browser() -> bool {
    default_handler_for_mime("x-scheme-handler/https")
        .map(|id| focus_handler_by_desktop_id(&id))
        .unwrap_or(false)
}

/// Bring the default handler for `path`'s MIME type to the foreground. Used
/// after xdg-open <file> on Sway/i3, where activation alone doesn't raise
/// the handler window.
#[cfg(target_os = "linux")]
fn focus_file_handler(path: &str) -> bool {
    let Ok(output) = host_command("xdg-mime")
        .args(["query", "filetype", path])
        .output()
    else {
        return false;
    };
    let mime = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if mime.is_empty() {
        return false;
    }
    let Some(desktop_id) = default_handler_for_mime(&mime) else {
        return false;
    };
    focus_handler_by_desktop_id(&desktop_id)
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
            if let Ok(output) = host_command("swaymsg")
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

    // i3 window manager - use i3-msg exclusively (i3 ignores raw X11
    // _NET_ACTIVE_WINDOW messages, so the x11rb fallback would report
    // success without actually focusing).  Try both class and instance
    // criteria: GTK apps often set instance to the reverse-DNS app ID
    // (e.g. "org.pwmt.zathura") while class is the short name ("Zathura").
    if std::env::var("I3SOCK").is_ok() {
        for criterion in [
            format!("[class=\"(?i){wm_class}\"] focus"),
            format!("[instance=\"(?i){wm_class}\"] focus"),
        ] {
            if let Ok(output) = host_command("i3-msg")
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

    // niri: no swaymsg/i3-msg compatible IPC and no X11 windows to activate.
    if crate::platform::linux::wm::is_niri() {
        return try_focus_niri(&[wm_class]);
    }

    // KDE Wayland: the x11rb path below only sees XWayland clients (under
    // the AppImage the Look window itself is XWayland), so native Wayland
    // windows are invisible to it. Go through KWin's scripting D-Bus.
    if crate::platform::linux::transparency::is_wayland() && crate::platform::linux::wm::is_kde() {
        return crate::platform::linux::kde_focus::try_focus(&[wm_class]);
    }

    // Non-i3: try i3-msg anyway (might be running), then x11rb fallback.
    if let Ok(output) = host_command("i3-msg")
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
    // segment ("zathura") - many apps use the short name as WM_CLASS even
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
    if crate::platform::linux::wm::is_niri() {
        return try_focus_niri(candidates);
    }
    // KDE Wayland: KWin scripting D-Bus (no GNOME Shell, no wlr protocol)
    if crate::platform::linux::wm::is_kde() {
        return crate::platform::linux::kde_focus::try_focus(candidates);
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
        if let Ok(output) = host_command("swaymsg")
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

/// niri: its own IPC is the only path that scrolls to the window's workspace.
/// `wlr-foreign-toplevel` activation (which niri also advertises) raises the
/// window without moving the view, leaving the user on an empty workspace, so
/// it is only a fallback for the socket being unavailable.
#[cfg(target_os = "linux")]
fn try_focus_niri(candidates: &[&str]) -> bool {
    if crate::platform::linux::niri::try_focus(candidates) {
        return true;
    }
    candidates
        .iter()
        .any(|id| crate::platform::linux::wlr_focus::try_focus(id))
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
    let _ = host_command("hyprctl")
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
    let Ok(output) = host_command("hyprctl")
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
    let Ok(output) = host_command("hyprctl")
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
