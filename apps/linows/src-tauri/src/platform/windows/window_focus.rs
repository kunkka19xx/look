//! Focus an existing app window before launching a fresh instance.
//!
//! Mirrors the WinUI3 reference (`apps/windows/LauncherApp/Services/ActionDispatcher.cs`
//! `TryActivateExistingAppWindow`). Without this hook, every Enter on a
//! search result hands off to `ShellExecuteW` / `open::that`, which spins
//! up a new process even when the app already owns a visible window —
//! noisy and unexpected for users who think of Look as a window switcher.
//!
//! Two matching strategies:
//! - **UWP** (`shell:AppsFolder\<PackageFamilyName>!<AppId>`): walk all
//!   processes whose exe lives under `\WindowsApps\`, compare each one's
//!   AUMID (`GetApplicationUserModelId`) against the target.
//! - **Win32** (`.exe` / resolved `.lnk` target): case-insensitive full
//!   exe-path comparison against `QueryFullProcessImageNameW`, pre-filtered
//!   by basename to skip the kernel call on most processes.
//!
//! Once a PID matches, `EnumWindows` finds the first visible top-level
//! window for it; `SetForegroundWindow` (with `SW_RESTORE` for minimized
//! windows) raises it. Call this **before** hiding Look's own window —
//! `SetForegroundWindow` only succeeds while we still hold foreground.

use windows::Win32::Foundation::{CloseHandle, FALSE, HANDLE, HWND, LPARAM, MAX_PATH, TRUE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, IPersistFile,
    STGM_READ,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SW_RESTORE,
    SetForegroundWindow, ShowWindowAsync,
};
use windows::core::BOOL;
use windows::core::{HSTRING, Interface, PWSTR};

// GetApplicationUserModelId lives in kernel32 but the windows crate doesn't
// expose it under the features we already pull in. Declaring it directly
// avoids dragging another feature flag in just for one symbol.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetApplicationUserModelId(
        h_process: HANDLE,
        application_user_model_id_length: *mut u32,
        application_user_model_id: *mut u16,
    ) -> i32;
}

// The probe call returns this when the buffer is too small — the only path
// that tells us the AUMID exists and how long it is. APPMODEL_ERROR_NO_APPLICATION
// (15700) and success-with-length-zero both mean "no AUMID, give up".
const ERROR_INSUFFICIENT_BUFFER: i32 = 122;

const UWP_PREFIX_BACK: &str = "shell:AppsFolder\\";
const UWP_PREFIX_FWD: &str = "shell:AppsFolder/";

pub(crate) fn try_focus_existing(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return false;
    }

    let target = match resolve_target(trimmed) {
        Some(t) => t,
        None => return false,
    };

    // Collect every PID that matches, not just the first. Modern apps spawn
    // helper processes that share the same AUMID or live under the same
    // package dir but don't own a window — Windows Terminal pulls in
    // OpenConsole.exe and shell children, any of which can enumerate first.
    // Picking only the first PID then giving up if it has no window meant
    // Terminal silently fell through to "launch a new instance".
    let pids = match &target {
        Target::Aumid(aumid) => find_uwp_pids_by_aumid(aumid),
        Target::ExePath(exe) => find_pids_by_exe(exe),
    };
    if pids.is_empty() {
        return false;
    }

    let Some(hwnd) = find_main_window_for_pids(&pids) else {
        return false;
    };

    activate_window(hwnd);
    true
}

enum Target {
    Aumid(String),
    ExePath(String),
}

fn resolve_target(path: &str) -> Option<Target> {
    if let Some(rest) = path
        .strip_prefix(UWP_PREFIX_BACK)
        .or_else(|| path.strip_prefix(UWP_PREFIX_FWD))
    {
        let aumid = rest.trim();
        // A valid AUMID is `<PackageFamilyName>!<AppId>`. Without the bang
        // it's a package family name only — nothing reliable to match against.
        if aumid.contains('!') {
            return Some(Target::Aumid(aumid.to_string()));
        }
        return None;
    }

    let normalized = normalize_path(path);
    if normalized.is_empty() {
        return None;
    }
    let lower = normalized.to_lowercase();
    if lower.ends_with(".lnk") {
        return resolve_lnk_target(&normalized).map(Target::ExePath);
    }
    if lower.ends_with(".exe") {
        return Some(Target::ExePath(normalized));
    }
    None
}

fn normalize_path(path: &str) -> String {
    let mut out = path.replace('/', "\\").trim().to_string();
    // Preserve drive roots like "C:\"; only strip trailing slash for longer paths.
    while out.ends_with('\\') && out.len() > 3 {
        out.pop();
    }
    out
}

fn resolve_lnk_target(lnk_path: &str) -> Option<String> {
    unsafe {
        // RPC_E_CHANGED_MODE is harmless — the icon resolver may have already
        // CoInit'd this thread in a compatible apartment model.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        persist.Load(&HSTRING::from(lnk_path), STGM_READ).ok()?;

        let mut buf = [0u16; 260];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        if len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

fn find_uwp_pids_by_aumid(target_aumid: &str) -> Vec<u32> {
    enumerate_processes(|pid, _basename| {
        let Some(handle) = open_process(pid) else {
            return false;
        };
        let matched = (|| {
            let path = query_full_image_name(handle)?;
            // Pre-filter: UWP entrypoints live under WindowsApps. Skip the
            // AUMID probe on the thousands of regular Win32 processes.
            if !path.to_lowercase().contains("\\windowsapps\\") {
                return None;
            }
            let aumid = get_process_aumid(handle)?;
            if aumid.eq_ignore_ascii_case(target_aumid) {
                Some(())
            } else {
                None
            }
        })()
        .is_some();
        unsafe {
            let _ = CloseHandle(handle);
        }
        matched
    })
}

fn find_pids_by_exe(target_exe_path: &str) -> Vec<u32> {
    let target_norm = normalize_path(target_exe_path).to_lowercase();
    let target_basename = std::path::Path::new(&target_norm)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    enumerate_processes(|pid, basename| {
        // szExeFile only gives basename, but that's enough to skip the
        // OpenProcess + QueryFullProcessImageName syscalls for non-candidates.
        if !target_basename.is_empty() && !basename.eq_ignore_ascii_case(&target_basename) {
            return false;
        }
        let Some(handle) = open_process(pid) else {
            return false;
        };
        let path = query_full_image_name(handle);
        unsafe {
            let _ = CloseHandle(handle);
        }
        match path {
            Some(p) => normalize_path(&p).to_lowercase() == target_norm,
            None => false,
        }
    })
}

/// Walk Toolhelp32 process snapshot, returning every PID for which
/// `predicate(pid, exe_basename)` is true.
fn enumerate_processes<F: FnMut(u32, &str) -> bool>(mut predicate: F) -> Vec<u32> {
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
                    if predicate(pid, &name) {
                        out.push(pid);
                    }
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

fn open_process(pid: u32) -> Option<HANDLE> {
    // PROCESS_QUERY_LIMITED_INFORMATION is the minimum right needed for both
    // QueryFullProcessImageNameW and GetApplicationUserModelId, and it works
    // against processes we couldn't open with PROCESS_QUERY_INFORMATION
    // (notably ones running at a higher integrity level).
    unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok() }
}

fn query_full_image_name(handle: HANDLE) -> Option<String> {
    // Long paths can exceed MAX_PATH; double the buffer to handle that without
    // a retry loop. PROCESS_NAME_WIN32 returns DOS-style paths.
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

fn get_process_aumid(handle: HANDLE) -> Option<String> {
    let mut len: u32 = 0;
    let probe = unsafe { GetApplicationUserModelId(handle, &mut len, std::ptr::null_mut()) };
    if probe != ERROR_INSUFFICIENT_BUFFER || len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    let rc = unsafe { GetApplicationUserModelId(handle, &mut len, buf.as_mut_ptr()) };
    if rc != 0 || len == 0 {
        return None;
    }
    // Returned length includes the terminating null; drop it before lossy decode.
    let mut actual = len as usize;
    if actual > 0 && buf[actual - 1] == 0 {
        actual -= 1;
    }
    if actual == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..actual]))
}

fn find_main_window_for_pids(pids: &[u32]) -> Option<HWND> {
    struct Search<'a> {
        pids: &'a [u32],
        hwnd: HWND,
    }
    let mut search = Search {
        pids,
        hwnd: HWND::default(),
    };

    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let s = unsafe { &mut *(lparam.0 as *mut Search) };
        let mut wnd_pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut wnd_pid)) };
        if !s.pids.contains(&wnd_pid) {
            return TRUE;
        }
        if !unsafe { IsWindowVisible(hwnd).as_bool() } {
            return TRUE;
        }
        s.hwnd = hwnd;
        FALSE
    }

    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut search as *mut _ as isize));
    }
    if search.hwnd.0.is_null() {
        None
    } else {
        Some(search.hwnd)
    }
}

fn activate_window(hwnd: HWND) {
    unsafe {
        // SW_RESTORE on a non-minimized window can un-maximize it (Edge losing
        // F11/fullscreen). Only restore when actually minimized; otherwise
        // SetForegroundWindow alone is enough to raise the window.
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
}
