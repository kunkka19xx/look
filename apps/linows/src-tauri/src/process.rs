//! Cross-platform process listing / kill Tauri commands. Real per-OS
//! implementations live in `platform::linux::process` and
//! `platform::windows::process`.

use serde::Serialize;
use std::sync::{Mutex, OnceLock};

#[derive(Serialize, Clone)]
pub struct RunningApp {
    pub name: String,
    pub pid: u32,
    pub desktop_id: Option<String>,
    pub exec: Option<String>,
}

/// One row in the `ps"` finder: a raw process, not desktop-matched like
/// `RunningApp`. Kept minimal so enumeration stays cheap; richer fields are
/// fetched per-selection via `process_detail`.
#[derive(Serialize, Clone)]
pub struct ProcRow {
    pub name: String,
    pub pid: u32,
    /// `.desktop` path when this process belongs to a known app, so the finder
    /// can show the app icon; `None` falls back to the generic process glyph.
    pub icon_source: Option<String>,
    /// Listening TCP ports owned by this process, sorted. Shown in the row and
    /// matched against a numeric query so `ps"` doubles as a find-by-port.
    pub ports: Vec<u16>,
}

/// Per-process detail for the `ps"` preview pane. All fields are cheap single
/// `/proc` reads; `start_epoch` is Unix seconds (formatted on the frontend).
#[derive(Serialize, Clone)]
pub struct ProcDetail {
    pub cmdline: String,
    pub rss_kb: u64,
    pub user: String,
    pub ppid: u32,
    pub start_epoch: Option<u64>,
}

/// Cap on rows returned to the finder: enough to scroll, few enough to render
/// instantly. The raw snapshot behind it can hold every user process.
const PROC_RESULT_LIMIT: usize = 50;

/// Snapshot of every user process, enumerated once per `ps"` session (or after
/// a kill) and re-scored per keystroke. Avoids walking `/proc` on every stroke.
fn snapshot() -> &'static Mutex<Vec<ProcRow>> {
    static SNAPSHOT: OnceLock<Mutex<Vec<ProcRow>>> = OnceLock::new();
    SNAPSHOT.get_or_init(|| Mutex::new(Vec::new()))
}

fn list_all_raw() -> Vec<ProcRow> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::list_all()
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::process::list_all()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// Scores for numeric-query matches, ranked above fuzzy name matches so an
/// intentional port/PID lookup wins. Exact port beats a partial digit match.
const PORT_EXACT_SCORE: i64 = i64::MAX / 2;
const NUMERIC_PARTIAL_SCORE: i64 = i64::MAX / 4;
/// PID-only matches rank below every name match (fuzzy scores are small).
const PID_PARTIAL_SCORE: i64 = i64::MIN / 2;

/// Fuzzy-find running processes for the `ps"` prefix. `refresh` re-enumerates
/// `/proc` (mode entry or post-kill); otherwise the cached snapshot is scored.
/// Matches on process name via `look-matching`; a numeric query also matches a
/// listening port (find-by-port) or the PID. Async + `spawn_blocking` because a
/// refresh scans socket fds and shouldn't touch the main thread.
#[tauri::command]
pub async fn search_processes(query: String, refresh: bool) -> Vec<ProcRow> {
    tauri::async_runtime::spawn_blocking(move || search_processes_blocking(&query, refresh))
        .await
        .unwrap_or_default()
}

fn search_processes_blocking(query: &str, refresh: bool) -> Vec<ProcRow> {
    let mut snap = snapshot().lock().unwrap();
    if refresh || snap.is_empty() {
        *snap = list_all_raw();
    }

    let trimmed = query.trim();
    if trimmed.is_empty() {
        let mut rows = snap.clone();
        rows.sort_by_key(|r| r.name.to_lowercase());
        rows.truncate(PROC_RESULT_LIMIT);
        return rows;
    }

    // fuzzy_score is case-sensitive; lowercase both sides (title is lowercased
    // in score_row), matching the engine's normalized-query convention.
    let lowered = trimmed.to_lowercase();
    let prepared = look_matching::prepare_query(&lowered);
    let is_numeric = trimmed.chars().all(|c| c.is_ascii_digit());
    let exact_port = trimmed.parse::<u16>().ok();

    let mut scored: Vec<(i64, &ProcRow)> = snap
        .iter()
        .filter_map(|row| {
            score_row(row, trimmed, &prepared, is_numeric, exact_port).map(|s| (s, row))
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()))
    });
    scored.truncate(PROC_RESULT_LIMIT);
    scored.into_iter().map(|(_, row)| row.clone()).collect()
}

/// Shared scoring for both process finders (`ps"` and `kill`). A numeric query
/// matches a listening port (exact beats partial) or the PID; every query also
/// fuzzy-matches the name. `None` when nothing matches. Pass an empty `ports`
/// slice for sources that don't carry ports (e.g. desktop-app rows).
fn score_process(
    name: &str,
    ports: &[u16],
    pid: u32,
    trimmed: &str,
    prepared: &look_matching::PreparedQuery<'_>,
    is_numeric: bool,
    exact_port: Option<u16>,
) -> Option<i64> {
    if is_numeric {
        if exact_port.is_some_and(|p| ports.contains(&p)) {
            return Some(PORT_EXACT_SCORE);
        }
        if ports.iter().any(|p| p.to_string().contains(trimmed)) {
            return Some(NUMERIC_PARTIAL_SCORE);
        }
        if pid.to_string().contains(trimmed) {
            return Some(PID_PARTIAL_SCORE);
        }
    }
    // Name fuzzy still runs for numeric queries: a name can contain digits.
    look_matching::fuzzy_score_prepared(prepared, &name.to_lowercase())
}

fn score_row(
    row: &ProcRow,
    trimmed: &str,
    prepared: &look_matching::PreparedQuery<'_>,
    is_numeric: bool,
    exact_port: Option<u16>,
) -> Option<i64> {
    score_process(
        &row.name, &row.ports, row.pid, trimmed, prepared, is_numeric, exact_port,
    )
}

/// A row in the `kill` command: either a desktop app (nice name + icon) or a
/// raw process. Apps rank above processes so `kill firefox` surfaces the app.
#[derive(Serialize, Clone)]
pub struct KillTarget {
    pub name: String,
    pub pid: u32,
    pub is_app: bool,
    pub desktop_id: Option<String>,
    pub exec: Option<String>,
}

const KILL_RESULT_LIMIT: usize = 60;

/// Fuzzy-find kill targets: apps first (matched on display name), then any other
/// process (matched on `/proc` name), deduped by PID. Empty query lists apps
/// only, matching the panel's default view. Async + `spawn_blocking`: it walks
/// `/proc` fresh each query, so it stays off the main thread.
#[tauri::command]
pub async fn search_kill_targets(query: String) -> Vec<KillTarget> {
    tauri::async_runtime::spawn_blocking(move || {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return list_processes()
                .into_iter()
                .take(KILL_RESULT_LIMIT)
                .map(app_target)
                .collect();
        }
        rank_kill_targets(list_processes(), list_all_raw(), trimmed)
    })
    .await
    .unwrap_or_default()
}

fn kill_hit_order(a: &(i64, KillTarget), b: &(i64, KillTarget)) -> std::cmp::Ordering {
    b.0.cmp(&a.0)
        .then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()))
}

fn rank_kill_targets(apps: Vec<RunningApp>, procs: Vec<ProcRow>, query: &str) -> Vec<KillTarget> {
    // Lowercase both sides: fuzzy_score is case-sensitive and app names are
    // Capitalized ("Firefox") while queries are typed lowercase.
    let lowered = query.to_lowercase();
    let prepared = look_matching::prepare_query(&lowered);
    let is_numeric = query.chars().all(|c| c.is_ascii_digit());
    let exact_port = query.parse::<u16>().ok();

    // Apps carry no ports, so they match by name only; a numeric port/PID query
    // resolves through the process rows below (which own the port data).
    let mut app_pids = std::collections::HashSet::new();
    let mut app_hits: Vec<(i64, KillTarget)> = apps
        .into_iter()
        .filter_map(|a| {
            look_matching::fuzzy_score_prepared(&prepared, &a.name.to_lowercase()).map(|s| {
                app_pids.insert(a.pid);
                (s, app_target(a))
            })
        })
        .collect();
    app_hits.sort_by(kill_hit_order);

    // Processes already shown as an app (its representative PID) are skipped.
    let mut proc_hits: Vec<(i64, KillTarget)> = procs
        .into_iter()
        .filter(|r| !app_pids.contains(&r.pid))
        .filter_map(|r| {
            score_process(
                &r.name, &r.ports, r.pid, query, &prepared, is_numeric, exact_port,
            )
            .map(|s| (s, proc_target(r)))
        })
        .collect();
    proc_hits.sort_by(kill_hit_order);

    let mut out: Vec<KillTarget> = app_hits.into_iter().map(|(_, t)| t).collect();
    out.extend(proc_hits.into_iter().map(|(_, t)| t));
    out.truncate(KILL_RESULT_LIMIT);
    out
}

fn app_target(a: RunningApp) -> KillTarget {
    KillTarget {
        name: a.name,
        pid: a.pid,
        is_app: true,
        desktop_id: a.desktop_id,
        exec: a.exec,
    }
}

fn proc_target(r: ProcRow) -> KillTarget {
    KillTarget {
        name: r.name,
        pid: r.pid,
        is_app: false,
        // App-backed processes still carry the app icon via their desktop path.
        desktop_id: r.icon_source.map(|p| format!("app:{p}")),
        exec: None,
    }
}

#[tauri::command]
pub fn process_detail(pid: u32) -> Option<ProcDetail> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::detail(pid)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = pid;
        None
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = pid;
        None
    }
}

/// Sample CPU usage for one process over a short interval. On-demand (bound to
/// Enter in `ps"`), never per-selection, so the sampling cost is only paid when
/// asked. Percentage is relative to a single core (may exceed 100, like `top`).
/// Async + `spawn_blocking` so the sampling sleep never stalls the main thread.
#[tauri::command]
pub async fn process_cpu(pid: u32) -> Option<f64> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "linux")]
        {
            crate::platform::linux::process::cpu(pid)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = pid;
            None
        }
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
pub fn list_processes() -> Vec<RunningApp> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::list()
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::process::list()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// List running GUI apps (apps with visible windows).
/// Unlike `list_processes` (used by /kill), this filters out background
/// services, terminal apps, and input methods - only switchable apps remain.
#[tauri::command]
pub fn list_running_apps() -> Vec<RunningApp> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::list_gui()
    }

    #[cfg(target_os = "windows")]
    {
        // Switcher-specific view: includes UWP apps (Settings, Calculator, …)
        // hosted by ApplicationFrameHost that list()/kill deliberately hides.
        crate::platform::windows::process::list_gui()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Vec::new()
    }
}

/// Activate (focus) a running app's window. On Linux, dispatches to the
/// same try_focus_existing chain used by open_path (compositor-aware).
/// On Windows, uses SetForegroundWindow via the window_focus module.
#[tauri::command]
pub fn activate_running_app(
    window: tauri::WebviewWindow,
    pid: u32,
    desktop_id: Option<String>,
    exec: Option<String>,
) -> Result<bool, String> {
    let _ = (&pid, &exec); // may be unused on some platforms

    #[cfg(target_os = "linux")]
    {
        // Try focus via desktop file metadata (WM_CLASS, app_id, etc.)
        if let Some(ref id) = desktop_id
            && let Some(desktop_path) = id.strip_prefix("app:")
            && crate::commands::try_focus_existing_pub(desktop_path)
        {
            let _ = window.hide();
            return Ok(true);
        }
        // Fallback: try focusing by exec binary name
        if let Some(ref exec_str) = exec {
            let bin = std::path::Path::new(
                exec_str
                    .split_whitespace()
                    .find(|t| !t.contains('='))
                    .unwrap_or(exec_str),
            )
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
            if !bin.is_empty() && crate::commands::try_focus_window_pub(bin) {
                let _ = window.hide();
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(ref id) = desktop_id {
            // UWP windows (Settings, …) are addressed by HWND: several share one
            // ApplicationFrameHost PID, so exe-path matching can't disambiguate.
            if let Some(raw) = id.strip_prefix("hwnd:") {
                if let Ok(h) = raw.parse::<isize>()
                    && crate::platform::windows::window_focus::focus_hwnd(h)
                {
                    let _ = window.hide();
                    return Ok(true);
                }
                return Ok(false);
            }
            if let Some(path) = id.strip_prefix("app:")
                && crate::platform::windows::window_focus::try_focus_existing(path)
            {
                let _ = window.hide();
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (window, desktop_id);
        Ok(false)
    }
}

#[tauri::command]
pub fn kill_process(pid: u32) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::process::kill(pid)
    }

    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::process::kill(pid)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = pid;
        Err("kill not supported on this platform".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, pid: u32, ports: &[u16]) -> ProcRow {
        ProcRow {
            name: name.to_string(),
            pid,
            icon_source: None,
            ports: ports.to_vec(),
        }
    }

    fn score(row: &ProcRow, query: &str) -> Option<i64> {
        let trimmed = query.trim();
        let lowered = trimmed.to_lowercase();
        let prepared = look_matching::prepare_query(&lowered);
        let is_numeric = trimmed.chars().all(|c| c.is_ascii_digit());
        let exact_port = trimmed.parse::<u16>().ok();
        score_row(row, trimmed, &prepared, is_numeric, exact_port)
    }

    #[test]
    fn exact_port_beats_partial_and_name() {
        let server = row("node", 4321, &[3000]);
        let other = row("node", 30001, &[]); // "3000" is a PID substring only
        let named = row("proc3000", 99, &[]); // name contains the digits
        let s_exact = score(&server, "3000").unwrap();
        assert_eq!(s_exact, PORT_EXACT_SCORE);
        assert!(s_exact > score(&other, "3000").unwrap());
        assert!(s_exact > score(&named, "3000").unwrap());
    }

    #[test]
    fn partial_port_ranks_above_pid_only() {
        let by_port = row("a", 1, &[3000]); // "300" is a substring of the port
        let by_pid = row("b", 3009, &[]); // "300" is a substring of the PID
        assert_eq!(score(&by_port, "300"), Some(NUMERIC_PARTIAL_SCORE));
        assert_eq!(score(&by_pid, "300"), Some(PID_PARTIAL_SCORE));
        assert!(score(&by_port, "300").unwrap() > score(&by_pid, "300").unwrap());
    }

    #[test]
    fn name_query_still_fuzzy_matches() {
        let ff = row("firefox", 100, &[]);
        assert!(score(&ff, "fire").is_some());
        assert!(score(&ff, "zzq").is_none());
    }

    #[test]
    fn oversized_port_number_wont_panic_and_matches_nothing() {
        // 99999 > u16::MAX: no exact port, no substring, no PID -> None.
        let r = row("svc", 42, &[8080]);
        assert_eq!(score(&r, "99999"), None);
    }

    fn app(name: &str, pid: u32) -> RunningApp {
        RunningApp {
            name: name.to_string(),
            pid,
            desktop_id: Some(format!("app:/x/{name}.desktop")),
            exec: Some(name.to_string()),
        }
    }

    #[test]
    fn kill_targets_rank_apps_before_processes() {
        let apps = vec![app("Firefox", 100)];
        // pid 100 is the app's own PID (deduped); 200 is a stray "firefox" proc.
        let procs = vec![row("firefox", 100, &[]), row("firefox", 200, &[])];
        let out = rank_kill_targets(apps, procs, "fire");
        assert_eq!(out.len(), 2, "app + the non-dup process");
        assert!(out[0].is_app, "app ranks first");
        assert_eq!(out[0].pid, 100);
        assert!(!out[1].is_app);
        assert_eq!(out[1].pid, 200);
    }

    #[test]
    fn kill_targets_non_match_excluded() {
        let out = rank_kill_targets(vec![app("Firefox", 1)], vec![row("bash", 2, &[])], "zzq");
        assert!(out.is_empty());
    }

    #[test]
    fn kill_targets_match_by_port_and_pid() {
        let apps = vec![app("Firefox", 1)];
        let procs = vec![row("node", 4321, &[3000]), row("bash", 900, &[])];
        // A bare port number finds its owner via the process rows (no `:` prefix).
        let by_port = rank_kill_targets(apps.clone(), procs.clone(), "3000");
        assert_eq!(by_port.len(), 1);
        assert_eq!(by_port[0].pid, 4321);
        // A bare PID finds the process directly.
        let by_pid = rank_kill_targets(apps, procs, "900");
        assert_eq!(by_pid.len(), 1);
        assert_eq!(by_pid[0].pid, 900);
    }
}
