//! Windows process listing / kill. EnumWindows tags every visible top-level
//! window with its owning PID, Toolhelp32 walks the full process list,
//! GetExtendedTcpTable maps PIDs to listening ports. Filtering
//! (system-noise names, \WindowsApps\, \SystemApps\, \ImmersiveControlPanel\)
//! is bypassed for any process that owns a visible window, so UWP apps like
//! Windows Terminal still show up.

use crate::process::{ProcDetail, ProcRow, RunningApp};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation};
use windows::Win32::Foundation::{
    CloseHandle, FILETIME, HANDLE, HWND, LPARAM, MAX_PATH, TRUE, UNICODE_STRING,
};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP_STATE_LISTEN, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountSidW, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, GetProcessTimes, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
    TerminateProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GWL_EXSTYLE, GetShellWindow, GetWindow, GetWindowLongW,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PWSTR};

const AF_INET: u32 = 2;
const AF_INET6: u32 = 23;

/// FILETIME epoch (1601-01-01) to Unix epoch (1970-01-01), in 100 ns ticks.
const FILETIME_UNIX_DELTA: u64 = 116_444_736_000_000_000;
/// 100 ns ticks per second - FILETIME's unit for both wall-clock and CPU time.
const FILETIME_TICKS_PER_SEC: f64 = 1e7;

/// CPU sampling window: long enough for a stable delta, short enough to feel
/// instant when triggered from `ps"` Enter. Matches the Linux/macOS ports.
const CPU_SAMPLE_MS: u64 = 200;

pub(crate) fn list() -> Vec<RunningApp> {
    let visible = enumerate_visible_windows();
    let current_pid = unsafe { GetCurrentProcessId() };

    let mut windowed: Vec<RunningApp> = Vec::new();
    let mut fallback: Vec<RunningApp> = Vec::new();

    for ProcEntry { pid, name, .. } in enumerate_processes() {
        if pid == 0 || pid == 4 || pid == current_pid {
            continue;
        }
        let exe_path = resolve_full_path(pid).unwrap_or_default();
        let title = visible.get(&pid).cloned();
        let has_window = title.is_some();

        if should_hide(&name, &exe_path, has_window) {
            continue;
        }

        let title_str = title.unwrap_or_default();
        let display = resolve_display_name(&name, &exe_path, &title_str);
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
        } else if !is_system_noise(&name) {
            fallback.push(app);
        }
    }

    // Fall back to windowless processes only when nothing has a window -
    // otherwise the list is dominated by background helpers no one wants.
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

/// Raw process list for the `ps"` and `kill` finders. Every process (minus the
/// idle/system PIDs and ourselves), by Toolhelp basename, tagged with its
/// listening TCP ports so a numeric query matches by port or PID.
pub(crate) fn list_all() -> Vec<ProcRow> {
    let current_pid = unsafe { GetCurrentProcessId() };
    let mut ports_by_pid = listening_ports_by_pid();
    enumerate_processes()
        .into_iter()
        .filter(|e| e.pid != 0 && e.pid != 4 && e.pid != current_pid)
        .map(|e| ProcRow {
            name: e.name,
            pid: e.pid,
            // Windows has no `.desktop` files, so the icon source is the exe
            // path the shell resolver takes. Processes that deny the handle
            // fall through to the generic glyph.
            icon_source: resolve_full_path(e.pid).filter(|p| !p.is_empty()),
            ports: ports_by_pid.remove(&e.pid).unwrap_or_default(),
        })
        .collect()
}

// --- ps" preview detail ---

/// Per-selection detail for the `ps"` preview. Fields degrade independently, so
/// a process that refuses a handle still previews with its parent PID.
pub(crate) fn detail(pid: u32) -> Option<ProcDetail> {
    let mut out = ProcDetail {
        cmdline: String::new(),
        rss_kb: 0,
        user: String::new(),
        // Toolhelp needs no handle, so this survives a denied OpenProcess.
        ppid: parent_pid(pid),
        start_epoch: None,
    };
    with_process(pid, |h| {
        // Full argv, else the exe path: argv[0] without its arguments.
        out.cmdline = command_line(h)
            .or_else(|| query_full_image_name(h))
            .unwrap_or_default();
        out.rss_kb = private_bytes(h) / 1024;
        out.user = token_user(h).unwrap_or_default();
        out.start_epoch = creation_epoch(h);
    });
    Some(out)
}

/// Kernel + user CPU time sampled twice around a short sleep, as a percentage of
/// one core (may exceed 100, like Task Manager's per-core view). The caller runs
/// this off the main thread; nothing else may block on the sleep.
pub(crate) fn cpu(pid: u32) -> Option<f64> {
    with_process(pid, |h| {
        let before = cpu_ticks(h)?;
        let start = std::time::Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(CPU_SAMPLE_MS));
        let delta = cpu_ticks(h)?.saturating_sub(before) as f64;
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed <= 0.0 {
            return Some(0.0);
        }
        Some(100.0 * (delta / FILETIME_TICKS_PER_SEC) / elapsed)
    })
    .flatten()
}

/// Full command line (argv) of another process.
///
/// `ProcessCommandLineInformation` (Windows 8.1+) has ntdll walk the target's
/// PEB for us, so a query handle is enough. Walking it ourselves via
/// `ProcessBasicInformation` and `ReadProcessMemory` would also need
/// `PROCESS_VM_READ`, and breaks across the 32/64-bit boundary.
fn command_line(handle: HANDLE) -> Option<String> {
    let query = |buf: *mut core::ffi::c_void, len: u32, needed: &mut u32| unsafe {
        NtQueryInformationProcess(handle, ProcessCommandLineInformation, buf, len, needed).is_ok()
    };
    let mut needed = 0;
    query(std::ptr::null_mut(), 0, &mut needed); // sizing probe; fails by design
    if needed as usize <= std::mem::size_of::<UNICODE_STRING>() {
        return None;
    }
    let mut buf = aligned_buf(needed);
    let capacity = buf.len() * 8;
    if !query(buf.as_mut_ptr().cast(), capacity as u32, &mut needed) {
        return None;
    }

    let us = unsafe { *(buf.as_ptr() as *const UNICODE_STRING) };
    // ntdll points Buffer just past the header, inside our own allocation. Trust
    // it only after checking: a stray pointer here would be a wild read.
    let base = buf.as_ptr() as usize;
    let start = us.Buffer.0 as usize;
    if us.Length == 0 || start < base || start + us.Length as usize > base + capacity {
        return None;
    }
    let chars =
        unsafe { std::slice::from_raw_parts(us.Buffer.0 as *const u16, us.Length as usize / 2) };
    let cmdline = String::from_utf16_lossy(chars).trim().to_string();
    (!cmdline.is_empty()).then_some(cmdline)
}

/// Private commit charge in bytes: Task Manager's "Commit size", Process
/// Explorer's "Private Bytes". The closest Windows analog to macOS
/// `phys_footprint` / Linux USS. `WorkingSetSize` counts shared pages and reads
/// much higher, which confuses anyone cross-checking against Task Manager.
fn private_bytes(handle: HANDLE) -> u64 {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    let ok = unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            counters.cb,
        )
    };
    if ok.is_err() {
        return 0;
    }
    counters.PrivateUsage as u64
}

/// Account name owning the process, via its token SID. Bare name, no domain
/// prefix, matching the plain usernames the Linux and macOS ports show.
fn token_user(handle: HANDLE) -> Option<String> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(handle, TOKEN_QUERY, &mut token) }.ok()?;
    let buf = token_user_buf(token);
    unsafe {
        let _ = CloseHandle(token);
    }
    // Bound, not inlined below: the SID lives in this buffer, so it must outlive
    // the LookupAccountSidW call.
    let buf = buf?;

    let mut name = [0u16; 256];
    let mut name_len = name.len() as u32;
    let mut domain = [0u16; 256];
    let mut domain_len = domain.len() as u32;
    let mut kind = SID_NAME_USE::default();
    unsafe {
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        LookupAccountSidW(
            None,
            user.User.Sid,
            Some(PWSTR(name.as_mut_ptr())),
            &mut name_len,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domain_len,
            &mut kind,
        )
        .ok()?;
    }
    Some(String::from_utf16_lossy(&name[..name_len as usize]))
}

/// `TOKEN_USER` is a header plus a variable-length SID, so it is sized first.
fn token_user_buf(token: HANDLE) -> Option<Vec<u64>> {
    let mut needed = 0;
    unsafe {
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        if needed == 0 {
            return None;
        }
        let mut buf = aligned_buf(needed);
        GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr().cast()),
            (buf.len() * 8) as u32,
            &mut needed,
        )
        .ok()?;
        Some(buf)
    }
}

/// Zeroed buffer of at least `bytes`, u64-backed so a struct holding a pointer
/// (`UNICODE_STRING`, `TOKEN_USER`) lands aligned. Reading one out of a `Vec<u8>`
/// is UB: that allocation is only byte-aligned.
fn aligned_buf(bytes: u32) -> Vec<u64> {
    vec![0; bytes.div_ceil(8) as usize]
}

fn creation_epoch(handle: HANDLE) -> Option<u64> {
    filetime_u64(process_times(handle)?.0)
        .checked_sub(FILETIME_UNIX_DELTA)
        .map(|t| t / FILETIME_TICKS_PER_SEC as u64)
}

fn cpu_ticks(handle: HANDLE) -> Option<u64> {
    let (_, kernel, user) = process_times(handle)?;
    Some(filetime_u64(kernel) + filetime_u64(user))
}

/// (creation, kernel, user). Exit time is never useful here: the process is
/// live by construction.
fn process_times(handle: HANDLE) -> Option<(FILETIME, FILETIME, FILETIME)> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) }.ok()?;
    Some((creation, kernel, user))
}

fn filetime_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

// --- process enumeration ---

struct ProcEntry {
    pid: u32,
    ppid: u32,
    name: String,
}

fn enumerate_processes() -> Vec<ProcEntry> {
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
                if entry.th32ProcessID != 0 {
                    let end = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    out.push(ProcEntry {
                        pid: entry.th32ProcessID,
                        ppid: entry.th32ParentProcessID,
                        name: String::from_utf16_lossy(&entry.szExeFile[..end]),
                    });
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

/// 0 when the process is already gone.
fn parent_pid(pid: u32) -> u32 {
    enumerate_processes()
        .into_iter()
        .find(|e| e.pid == pid)
        .map_or(0, |e| e.ppid)
}

/// Run `f` with a query handle to `pid`, closing it afterwards. `None` when the
/// process refuses one: protected, another session, or already exited.
fn with_process<T>(pid: u32, f: impl FnOnce(HANDLE) -> T) -> Option<T> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let out = f(handle);
    unsafe {
        let _ = CloseHandle(handle);
    }
    Some(out)
}

fn resolve_full_path(pid: u32) -> Option<String> {
    with_process(pid, query_full_image_name).flatten()
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

// Reject-list - these descriptions are too generic to be useful and just
// shadow the process basename without adding information.
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

/// Map every PID to its listening TCP ports, built in one pass over the IPv4 and
/// IPv6 tables. Ports are sorted and deduped so a process is tagged consistently
/// regardless of the socket-table order.
fn listening_ports_by_pid() -> HashMap<u32, Vec<u16>> {
    let mut map: HashMap<u32, Vec<u16>> = HashMap::new();
    collect_listening_into(AF_INET, &mut map);
    collect_listening_into(AF_INET6, &mut map);
    for ports in map.values_mut() {
        ports.sort_unstable();
        ports.dedup();
    }
    map
}

fn collect_listening_into(af: u32, map: &mut HashMap<u32, Vec<u16>>) {
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
    let listen = MIB_TCP_STATE_LISTEN.0 as u32;

    let mut push = |pid: u32, port: u32| {
        if let Ok(p) = u16::try_from(port) {
            map.entry(pid).or_default().push(p);
        }
    };

    if af == AF_INET {
        let row_size = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
        for i in 0..num_entries {
            let off = 4 + i * row_size;
            if off + row_size > buf.len() {
                break;
            }
            let row: &MIB_TCPROW_OWNER_PID =
                unsafe { &*(buf.as_ptr().add(off) as *const MIB_TCPROW_OWNER_PID) };
            if row.dwState == listen {
                push(row.dwOwningPid, parse_port(row.dwLocalPort));
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
            if row.dwState == listen {
                push(row.dwOwningPid, parse_port(row.dwLocalPort));
            }
        }
    }
}

fn parse_port(port_field: u32) -> u32 {
    // dwLocalPort is the network-byte-order port in the low 16 bits, padded
    // with zeros in the high 16, so the port is a big-endian read of the low
    // halfword.
    let b = port_field.to_le_bytes();
    ((b[0] as u32) << 8) | b[1] as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_port_reads_network_byte_order() {
        // 8080 = 0x1F90; on the wire the bytes are 1F 90, stored little-endian
        // in the low halfword as 0x901F.
        assert_eq!(parse_port(0x0000_901F), 8080);
        assert_eq!(parse_port(0x0000_5000), 80);
    }

    #[test]
    fn self_detail_populates_every_field() {
        let me = unsafe { GetCurrentProcessId() };
        let d = detail(me).expect("detail for self");
        assert!(!d.cmdline.is_empty(), "self cmdline should not be empty");
        assert!(d.rss_kb > 0, "self private bytes should be non-zero");
        assert!(!d.user.is_empty(), "self token user should resolve");
        assert!(d.ppid > 0, "the test runner is our parent");
        assert!(d.start_epoch.is_some(), "self start time should resolve");
    }

    #[test]
    fn self_cpu_samples_non_negative() {
        let pct = cpu(unsafe { GetCurrentProcessId() }).expect("cpu for self");
        assert!(pct >= 0.0, "cpu percent must be non-negative, got {pct}");
    }

    #[test]
    fn list_all_tags_ports_and_excludes_self() {
        let me = unsafe { GetCurrentProcessId() };
        let rows = list_all();
        assert!(!rows.is_empty(), "Toolhelp must see some processes");
        assert!(!rows.iter().any(|r| r.pid == me), "own PID is filtered out");
        assert!(
            rows.iter().any(|r| r.icon_source.is_some()),
            "at least one process must expose an exe path for its icon"
        );
    }
}
