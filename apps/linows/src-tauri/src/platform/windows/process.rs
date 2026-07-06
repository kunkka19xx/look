//! Windows process listing / kill. Mirrors the WinUI3 reference at
//! apps/windows/LauncherApp/Commands/KillCommand.cs - EnumWindows tags every
//! visible top-level window with its owning PID, Toolhelp32 walks the full
//! process list, GetExtendedTcpTable does per-port lookups. Filtering
//! (system-noise names, \WindowsApps\, \SystemApps\, \ImmersiveControlPanel\)
//! is bypassed for any process that owns a visible window, so UWP apps like
//! Windows Terminal still show up.

use crate::process::RunningApp;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, MAX_PATH, TRUE};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP_STATE_LISTEN, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GWL_EXSTYLE, GetShellWindow, GetWindow, GetWindowLongW,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PWSTR};

const AF_INET: u32 = 2;
const AF_INET6: u32 = 23;

pub(crate) fn list() -> Vec<RunningApp> {
    let visible = enumerate_visible_windows();
    let current_pid = unsafe { GetCurrentProcessId() };

    let mut windowed: Vec<RunningApp> = Vec::new();
    let mut fallback: Vec<RunningApp> = Vec::new();

    for (pid, basename) in enumerate_processes() {
        if pid == 0 || pid == 4 || pid == current_pid {
            continue;
        }
        let exe_path = resolve_full_path(pid).unwrap_or_default();
        let title = visible.get(&pid).cloned();
        let has_window = title.is_some();

        if should_hide(&basename, &exe_path, has_window) {
            continue;
        }

        let title_str = title.unwrap_or_default();
        let display = resolve_display_name(&basename, &exe_path, &title_str);
        let app = RunningApp {
            name: display,
            pid,
            // The frontend (apps/linows/src/js/screens/commands/kill.js) only
            // requests an icon when desktop_id is truthy; mirror Linux's
            // "app:<path>" convention so the icon resolver gets called with
            // the exe path.
            desktop_id: (!exe_path.is_empty()).then(|| format!("app:{exe_path}")),
            exec: (!exe_path.is_empty()).then(|| exe_path.clone()),
        };

        if has_window {
            windowed.push(app);
        } else if !is_system_noise(&basename) {
            fallback.push(app);
        }
    }

    // WinUI3 only falls back to windowless processes when nothing has a window
    // - otherwise the list is dominated by background helpers no one wants.
    let mut apps = if !windowed.is_empty() {
        windowed
    } else {
        fallback
    };

    let mut seen: HashSet<u32> = HashSet::new();
    apps.retain(|a| seen.insert(a.pid));
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

pub(crate) fn list_on_port(port: u16) -> Vec<RunningApp> {
    let mut pids = collect_listening_pids(port);
    let current_pid = unsafe { GetCurrentProcessId() };
    pids.retain(|&pid| pid > 4 && pid != current_pid);
    pids.sort();
    pids.dedup();

    let visible = enumerate_visible_windows();
    let mut out = Vec::with_capacity(pids.len());
    for pid in pids {
        let exe_path = resolve_full_path(pid).unwrap_or_default();
        let basename = Path::new(&exe_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let title = visible.get(&pid).cloned().unwrap_or_default();
        let display = resolve_display_name(&basename, &exe_path, &title);
        out.push(RunningApp {
            name: display,
            pid,
            desktop_id: (!exe_path.is_empty()).then(|| format!("app:{exe_path}")),
            exec: (!exe_path.is_empty()).then(|| exe_path.clone()),
        });
    }
    out
}

/// Switcher view: one entry per switchable top-level window. Unlike `list()`
/// (the kill view), this surfaces UWP apps - Settings, Calculator, Photos, … -
/// whose visible windows are owned by `ApplicationFrameHost.exe` while their
/// real process is windowless, so `list()` hides them. UWP entries activate by
/// HWND (encoded in `desktop_id` as `hwnd:<handle>`) because several UWP apps
/// share one ApplicationFrameHost PID and exe-path matching can't tell them
/// apart. Normal apps keep the `app:<exe>` id and per-process dedup.
pub(crate) fn list_gui() -> Vec<RunningApp> {
    let current_pid = unsafe { GetCurrentProcessId() };
    let mut apps: Vec<RunningApp> = Vec::new();
    let mut seen_pids: HashSet<u32> = HashSet::new();

    for (hwnd_raw, pid, title) in enumerate_switchable_windows() {
        if pid == 0 || pid == 4 || pid == current_pid {
            continue;
        }
        let exe_path = resolve_full_path(pid).unwrap_or_default();
        let basename = Path::new(&exe_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let is_frame_host = basename.eq_ignore_ascii_case("applicationframehost.exe");
        // ApplicationFrameHost is the UWP UI host - keep it; drop other noise.
        if !is_frame_host && is_system_noise(&basename) {
            continue;
        }

        if is_frame_host {
            // The UWP window title ("Settings") is the app name (always non-empty
            // - enumerate_switchable_windows drops untitled windows). One entry
            // per window, activated by HWND.
            apps.push(RunningApp {
                name: title,
                pid,
                desktop_id: Some(format!("hwnd:{hwnd_raw}")),
                exec: (!exe_path.is_empty()).then(|| exe_path.clone()),
            });
        } else {
            if !seen_pids.insert(pid) {
                continue;
            }
            apps.push(RunningApp {
                name: resolve_display_name(&basename, &exe_path, &title),
                pid,
                desktop_id: (!exe_path.is_empty()).then(|| format!("app:{exe_path}")),
                exec: (!exe_path.is_empty()).then(|| exe_path.clone()),
            });
        }
    }

    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

pub(crate) fn kill(pid: u32) -> Result<String, String> {
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
            .map_err(|e| format!("OpenProcess({pid}) failed: {e}"))?;
        let terminate = TerminateProcess(handle, 1);
        let _ = CloseHandle(handle);
        terminate.map_err(|e| format!("TerminateProcess({pid}) failed: {e}"))?;
    }
    Ok(format!("Killed PID {pid}"))
}

// --- process enumeration ---

fn enumerate_processes() -> Vec<(u32, String)> {
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return out;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let pid = entry.th32ProcessID;
                if pid != 0 {
                    let name_len = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);
                    out.push((pid, name));
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    out
}

fn resolve_full_path(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    let result = query_full_image_name(handle);
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

fn query_full_image_name(handle: HANDLE) -> Option<String> {
    let mut buf = vec![0u16; (MAX_PATH as usize) * 2];
    let mut len = buf.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

// --- window enumeration ---

struct VisibleCtx {
    shell: HWND,
    map: HashMap<u32, String>,
}

fn enumerate_visible_windows() -> HashMap<u32, String> {
    let mut ctx = VisibleCtx {
        shell: unsafe { GetShellWindow() },
        map: HashMap::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(visible_cb), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.map
}

unsafe extern "system" fn visible_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam.0 as *mut VisibleCtx) };
    if hwnd == ctx.shell || !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return TRUE;
    }
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return TRUE;
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if copied <= 0 {
        return TRUE;
    }
    let title = String::from_utf16_lossy(&buf[..copied as usize])
        .trim()
        .to_string();
    if title.is_empty() {
        return TRUE;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return TRUE;
    }
    ctx.map
        .entry(pid)
        .and_modify(|t| {
            if title.len() > t.len() {
                *t = title.clone();
            }
        })
        .or_insert(title);
    TRUE
}

// --- switchable window enumeration (running-apps switcher) ---

struct SwitchCtx {
    shell: HWND,
    out: Vec<(isize, u32, String)>,
}

/// Visible, titled, top-level (un-owned, non-tool) windows: (HWND, PID, title).
/// Same selectivity as `window_focus::find_main_window_for_pids` so the listed
/// window is the one activation will raise.
fn enumerate_switchable_windows() -> Vec<(isize, u32, String)> {
    let mut ctx = SwitchCtx {
        shell: unsafe { GetShellWindow() },
        out: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(switchable_cb), LPARAM(&mut ctx as *mut _ as isize));
    }
    ctx.out
}

unsafe extern "system" fn switchable_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam.0 as *mut SwitchCtx) };
    if hwnd == ctx.shell || !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return TRUE;
    }
    // Owned windows are dialogs/popups, not the app's main frame.
    if let Ok(owner) = unsafe { GetWindow(hwnd, GW_OWNER) }
        && !owner.0.is_null()
    {
        return TRUE;
    }
    // WS_EX_TOOLWINDOW = floating palette, not a switchable app.
    let exstyle = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    if exstyle & WS_EX_TOOLWINDOW.0 != 0 {
        return TRUE;
    }
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return TRUE;
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if copied <= 0 {
        return TRUE;
    }
    let title = String::from_utf16_lossy(&buf[..copied as usize])
        .trim()
        .to_string();
    if title.is_empty() {
        return TRUE;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return TRUE;
    }
    ctx.out.push((hwnd.0 as isize, pid, title));
    TRUE
}

// --- filtering / naming ---

fn should_hide(name: &str, exe_path: &str, has_window: bool) -> bool {
    if is_system_noise(name) {
        return true;
    }
    if has_window {
        return false;
    }
    let lower = exe_path.to_lowercase().replace('/', "\\");
    lower.contains("\\windows\\systemapps\\")
        || lower.contains("\\windowsapps\\")
        || lower.contains("\\windows\\immersivecontrolpanel\\")
}

fn is_system_noise(name: &str) -> bool {
    let lower = name.to_lowercase();
    let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
    matches!(
        stem,
        "svchost"
            | "dwm"
            | "ctfmon"
            | "textinputhost"
            | "windowsinternal.composableshell.experiences.textinput.inputapp"
            | "searchhost"
            | "startmenuexperiencehost"
            | "shellexperiencehost"
            | "winlogon"
            | "fontdrvhost"
            | "csrss"
            | "smss"
            | "lsass"
            | "registry"
            | "services"
            | "sihost"
            | "taskhostw"
            // UWP frame wrapper - owns the visible window for Settings,
            // Calculator, etc., but the real app process is separate; killing
            // it tears down every UWP window at once.
            | "applicationframehost"
    )
}

fn resolve_display_name(process_name: &str, exe_path: &str, window_title: &str) -> String {
    if let Some(name) = derive_name_from_window_title(window_title) {
        return name;
    }
    let stem = process_name
        .trim()
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE");
    if !exe_path.is_empty()
        && let Some(desc) = cached_file_description(exe_path)
        && is_usable_description(&desc, stem)
    {
        return desc;
    }
    if stem.is_empty() {
        "Unknown".to_string()
    } else {
        stem.to_string()
    }
}

static FILE_DESCRIPTION_CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);

fn cached_file_description(exe_path: &str) -> Option<String> {
    let key = exe_path.to_lowercase();
    {
        let lock = FILE_DESCRIPTION_CACHE.lock().unwrap();
        if let Some(map) = lock.as_ref()
            && let Some(cached) = map.get(&key)
        {
            return cached.clone();
        }
    }
    let resolved = crate::platform::windows::version::read_file_description(exe_path);
    let mut lock = FILE_DESCRIPTION_CACHE.lock().unwrap();
    lock.get_or_insert_with(HashMap::new)
        .insert(key, resolved.clone());
    resolved
}

// WinUI3 reject-list - these descriptions are too generic to be useful and
// just shadow the process basename without adding information.
fn is_usable_description(desc: &str, stem: &str) -> bool {
    let trimmed = desc.trim();
    if trimmed.is_empty() {
        return false;
    }
    !trimmed.eq_ignore_ascii_case("Application")
        && !trimmed.eq_ignore_ascii_case("Program")
        && !trimmed.eq_ignore_ascii_case("Windows Software Development Kit")
        && !trimmed.eq_ignore_ascii_case(stem)
}

fn derive_name_from_window_title(title: &str) -> Option<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed
        .split(" - ")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() > 1 {
        let tail = parts[parts.len() - 1];
        if is_good_display_segment(tail) {
            return Some(tail.to_string());
        }
    }
    None
}

fn is_good_display_segment(value: &str) -> bool {
    let n = value.trim();
    let chars = n.chars().count();
    if !(3..=64).contains(&chars) {
        return false;
    }
    if n.contains('\\') || n.contains('/') || n.contains('|') {
        return false;
    }
    !n.eq_ignore_ascii_case("administrator") && !n.eq_ignore_ascii_case("running applications")
}

// --- per-port listing ---

fn collect_listening_pids(port: u16) -> Vec<u32> {
    let mut pids = Vec::new();
    collect_listening_pids_for(port, AF_INET, &mut pids);
    collect_listening_pids_for(port, AF_INET6, &mut pids);
    pids
}

fn collect_listening_pids_for(port: u16, af: u32, pids: &mut Vec<u32>) {
    let mut size: u32 = 0;
    unsafe {
        GetExtendedTcpTable(None, &mut size, false, af, TCP_TABLE_OWNER_PID_LISTENER, 0);
    }
    if size == 0 {
        return;
    }
    let mut buf = vec![0u8; size as usize];
    let rc = unsafe {
        GetExtendedTcpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut size,
            false,
            af,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if rc != 0 || buf.len() < 4 {
        return;
    }

    // Layout: u32 dwNumEntries followed by MIB_TCP{,6}ROW_OWNER_PID[N].
    // The trailing row struct has u32 alignment, so the table starts at offset 4.
    let num_entries = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let target = port as u32;
    let listen = MIB_TCP_STATE_LISTEN.0 as u32;

    if af == AF_INET {
        let row_size = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
        for i in 0..num_entries {
            let off = 4 + i * row_size;
            if off + row_size > buf.len() {
                break;
            }
            let row: &MIB_TCPROW_OWNER_PID =
                unsafe { &*(buf.as_ptr().add(off) as *const MIB_TCPROW_OWNER_PID) };
            if row.dwState == listen && parse_port(row.dwLocalPort) == target {
                pids.push(row.dwOwningPid);
            }
        }
    } else {
        let row_size = std::mem::size_of::<MIB_TCP6ROW_OWNER_PID>();
        for i in 0..num_entries {
            let off = 4 + i * row_size;
            if off + row_size > buf.len() {
                break;
            }
            let row: &MIB_TCP6ROW_OWNER_PID =
                unsafe { &*(buf.as_ptr().add(off) as *const MIB_TCP6ROW_OWNER_PID) };
            if row.dwState == listen && parse_port(row.dwLocalPort) == target {
                pids.push(row.dwOwningPid);
            }
        }
    }
}

fn parse_port(port_field: u32) -> u32 {
    // dwLocalPort is the network-byte-order port in the low 16 bits, padded
    // with zeros in the high 16. WinUI3 reads BitConverter.GetBytes(field)
    // and swaps the first two bytes; the equivalent here is a big-endian read
    // of the low halfword.
    let b = port_field.to_le_bytes();
    ((b[0] as u32) << 8) | b[1] as u32
}
