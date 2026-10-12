//! Read an executable's version from the Win32 VERSION resource.
//!
//! Mirrors what Explorer's Properties dialog shows on the Details tab and
//! what the macOS preview pane shows for `.app` bundles. Used by the
//! launcher's right-side preview to display "Version: 137.0.7204.92" under
//! a focused app result.
//!
//! Path handling:
//! - `.lnk` shortcuts are resolved to their target `.exe` via `IShellLinkW`
//!   (Start Menu entries are .lnk-based).
//! - `shell:AppsFolder\<AUMID>` (UWP) returns None - version lives in the
//!   AppX package manifest, not in a queryable file resource. Adding it
//!   needs a WinRT roundtrip; skipped for now.
//! - Anything else is fed straight to `GetFileVersionInfoW`.
//!
//! We pull the binary `VS_FIXEDFILEINFO` block (the `"\\"` subblock) rather
//! than the string `\StringFileInfo\<lang-codepage>\ProductVersion` path,
//! because the latter requires picking the right translation entry first
//! and many apps ship only the numeric block anyway. `Major.Minor.Build`
//! is the format Explorer shows; we trim a trailing `.0` revision.

use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, IPersistFile,
    STGM_READ,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{HSTRING, Interface, PCWSTR};

pub(crate) fn read(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    // UWP entries don't expose a Win32 version resource on the AUMID path.
    if trimmed.starts_with("shell:") {
        return None;
    }

    let normalized = trimmed.replace('/', "\\");
    let target = if normalized.to_lowercase().ends_with(".lnk") {
        resolve_lnk_target(&normalized)?
    } else {
        normalized
    };

    read_fixed_file_info(&target)
}

/// Read the localized `FileDescription` string from an exe's VERSION resource.
/// Used by the `/kill` screen to upgrade `WindowsTerminal` → "Windows
/// Terminal", `chrome` → "Google Chrome", etc.
pub(crate) fn read_file_description(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed.starts_with("shell:") {
        return None;
    }
    read_string_file_info(&trimmed.replace('/', "\\"), "FileDescription")
        .or_else(|| read_string_file_info(&trimmed.replace('/', "\\"), "ProductName"))
}

fn read_string_file_info(path: &str, key: &str) -> Option<String> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let pcwstr = PCWSTR(wide.as_ptr());

    unsafe {
        let size = GetFileVersionInfoSizeW(pcwstr, None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(pcwstr, None, size, buf.as_mut_ptr() as *mut _).ok()?;

        // \VarFileInfo\Translation → packed array of DWORDs, each (codepage << 16) | langid.
        let translation_path: Vec<u16> = r"\VarFileInfo\Translation"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len: u32 = 0;
        let ok = VerQueryValueW(
            buf.as_ptr() as *const _,
            PCWSTR(translation_path.as_ptr()),
            &mut ptr,
            &mut len,
        );
        if !ok.as_bool() || ptr.is_null() || len < 4 {
            return None;
        }

        // Walk every translation - some exes ship en-US first, others SDK-default first.
        // Take the first FileDescription we can read.
        let translations = std::slice::from_raw_parts(ptr as *const u32, (len / 4) as usize);
        for &translation in translations {
            let langid = translation & 0xFFFF;
            let codepage = (translation >> 16) & 0xFFFF;
            let subblock = format!(r"\StringFileInfo\{langid:04x}{codepage:04x}\{key}");
            let wide_sub: Vec<u16> = subblock.encode_utf16().chain(std::iter::once(0)).collect();
            let mut s_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut s_len: u32 = 0;
            let s_ok = VerQueryValueW(
                buf.as_ptr() as *const _,
                PCWSTR(wide_sub.as_ptr()),
                &mut s_ptr,
                &mut s_len,
            );
            if !s_ok.as_bool() || s_ptr.is_null() || s_len == 0 {
                continue;
            }
            // s_len is char count including null terminator.
            let str_slice = std::slice::from_raw_parts(s_ptr as *const u16, s_len as usize);
            let end = str_slice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(str_slice.len());
            if end == 0 {
                continue;
            }
            let s = String::from_utf16_lossy(&str_slice[..end])
                .trim()
                .to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
        None
    }
}

fn resolve_lnk_target(lnk_path: &str) -> Option<String> {
    unsafe {
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

fn read_fixed_file_info(path: &str) -> Option<String> {
    // GetFileVersionInfoW needs a null-terminated UTF-16 buffer kept alive
    // for the whole call chain.
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let pcwstr = PCWSTR(wide.as_ptr());

    unsafe {
        let size = GetFileVersionInfoSizeW(pcwstr, None);
        if size == 0 {
            return None;
        }

        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(pcwstr, None, size, buf.as_mut_ptr() as *mut _).ok()?;

        // "\\" pulls the root VS_FIXEDFILEINFO block - numeric version,
        // language-agnostic. Available on virtually every Win32 binary
        // that ships any version resource at all.
        let root: Vec<u16> = "\\".encode_utf16().chain(std::iter::once(0)).collect();
        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len: u32 = 0;
        let ok = VerQueryValueW(
            buf.as_ptr() as *const _,
            PCWSTR(root.as_ptr()),
            &mut ptr,
            &mut len,
        );
        if !ok.as_bool()
            || ptr.is_null()
            || (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
        {
            return None;
        }

        let info = &*(ptr as *const VS_FIXEDFILEINFO);
        let ms = info.dwFileVersionMS;
        let ls = info.dwFileVersionLS;
        let major = (ms >> 16) & 0xFFFF;
        let minor = ms & 0xFFFF;
        let build = (ls >> 16) & 0xFFFF;
        let revision = ls & 0xFFFF;

        // All-zero version resource → don't show "0.0.0" in the UI.
        if (major | minor | build | revision) == 0 {
            return None;
        }

        // Trim trailing zero revision - Explorer's Details tab does the same.
        let formatted = if revision == 0 {
            format!("{major}.{minor}.{build}")
        } else {
            format!("{major}.{minor}.{build}.{revision}")
        };
        Some(formatted)
    }
}
