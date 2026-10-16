//! Resolve Windows known folders (Desktop / Documents / Downloads / Pictures
//! / Videos / Music) via `SHGetKnownFolderPath`.
//!
//! Needed because modern Windows / OneDrive often redirects these out of
//! `%USERPROFILE%` (e.g. Desktop → `%USERPROFILE%\OneDrive\Desktop`), so
//! the JS-side `${USERPROFILE}\Desktop` synthesis points at a path that
//! literally doesn't exist on the disk and Explorer rejects "open".

use std::path::Path;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music, FOLDERID_Pictures,
    FOLDERID_Videos, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath,
};
use windows::core::GUID;

pub(crate) fn list() -> Vec<(String, String)> {
    let entries: [(&str, &GUID); 6] = [
        ("Desktop", &FOLDERID_Desktop),
        ("Documents", &FOLDERID_Documents),
        ("Downloads", &FOLDERID_Downloads),
        ("Pictures", &FOLDERID_Pictures),
        ("Videos", &FOLDERID_Videos),
        ("Music", &FOLDERID_Music),
    ];
    entries
        .iter()
        .filter_map(|(title, rfid)| {
            let path = resolve(rfid)?;
            // Skip folders that don't actually exist (uncommon but possible
            // when a known folder GUID is registered but the directory was
            // deleted manually).
            Path::new(&path)
                .is_dir()
                .then_some(((*title).to_string(), path))
        })
        .collect()
}

fn resolve(rfid: &GUID) -> Option<String> {
    unsafe {
        let pw = SHGetKnownFolderPath(rfid, KNOWN_FOLDER_FLAG(0), None).ok()?;
        if pw.0.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *pw.0.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(pw.0, len);
        let s = String::from_utf16_lossy(slice);
        CoTaskMemFree(Some(pw.0 as *mut _));
        Some(s)
    }
}
