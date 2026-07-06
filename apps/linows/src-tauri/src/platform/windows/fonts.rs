//! Enumerate installed font families on Windows via GDI.
//!
//! `EnumFontFamiliesExW` with a DEFAULT_CHARSET probe walks every installed
//! face the system makes available to GDI - the same set Office, Notepad,
//! and the Windows Settings font picker see. We collect unique typeface
//! family names from `LOGFONTW.lfFaceName` and return them sorted.
//!
//! `@`-prefixed face names are vertical-writing variants of CJK fonts
//! intended for vertical text layout; they're dropped so the picker shows
//! "MS Gothic" but not "@MS Gothic".

use std::collections::BTreeSet;

use windows::Win32::Foundation::LPARAM;
use windows::Win32::Graphics::Gdi::{
    DEFAULT_CHARSET, EnumFontFamiliesExW, GetDC, LOGFONTW, ReleaseDC, TEXTMETRICW,
};

pub(crate) fn list() -> Vec<String> {
    let mut families: BTreeSet<String> = BTreeSet::new();
    unsafe {
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return Vec::new();
        }

        let lf = LOGFONTW {
            lfCharSet: DEFAULT_CHARSET,
            ..Default::default()
        };

        EnumFontFamiliesExW(
            hdc,
            &lf,
            Some(enum_proc),
            LPARAM(&mut families as *mut _ as isize),
            0,
        );

        ReleaseDC(None, hdc);
    }
    families.into_iter().collect()
}

unsafe extern "system" fn enum_proc(
    lplf: *const LOGFONTW,
    _lptm: *const TEXTMETRICW,
    _font_type: u32,
    lparam: LPARAM,
) -> i32 {
    let families = unsafe { &mut *(lparam.0 as *mut BTreeSet<String>) };
    let face = unsafe { (*lplf).lfFaceName };

    // Vertical-writing aliases for CJK fonts (e.g. "@MS Gothic"). Hide them -
    // they're a layout artefact, not a separate font the user would pick.
    if face.first().copied() == Some(b'@' as u16) {
        return 1;
    }

    let len = face.iter().position(|&c| c == 0).unwrap_or(face.len());
    let name = String::from_utf16_lossy(&face[..len]);
    if !name.is_empty() {
        families.insert(name);
    }
    1 // continue enumeration
}
