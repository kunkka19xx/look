//! UWP / MSIX app enumeration via the `shell:AppsFolder` Shell namespace.
//!
//! Win11 ships Notepad, Calculator, Weather, Mail, Photos, etc. as packaged
//! apps that don't appear as `.lnk` shortcuts in Start Menu Programs. They are
//! reachable only via the Shell namespace at `shell:AppsFolder\{AUMID}`.
//! Ported from `apps/windows/LauncherApp/Services/UwpAppService.cs`.

use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, IEnumShellItems, IShellItem, SHCreateItemFromParsingName, SIGDN,
    SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING,
};
use windows::core::HSTRING;

pub(crate) struct UwpApp {
    pub title: String,
    pub aumid: String,
}

pub(crate) fn enumerate_apps_folder() -> Vec<UwpApp> {
    let mut out = Vec::new();
    unsafe {
        // Re-init is idempotent; RPC_E_CHANGED_MODE is harmless and ignored.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let folder: IShellItem =
            match SHCreateItemFromParsingName(&HSTRING::from("shell:AppsFolder"), None) {
                Ok(f) => f,
                Err(_) => return out,
            };

        let enumerator: IEnumShellItems = match folder.BindToHandler(None, &BHID_EnumItems) {
            Ok(e) => e,
            Err(_) => return out,
        };

        loop {
            let mut buf: [Option<IShellItem>; 1] = [None];
            let mut fetched: u32 = 0;
            if enumerator.Next(&mut buf, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(item) = buf[0].take() else { continue };

            let Some(name) = get_display_name(&item, SIGDN_NORMALDISPLAY) else {
                continue;
            };
            let Some(mut aumid) = get_display_name(&item, SIGDN_PARENTRELATIVEPARSING) else {
                continue;
            };

            // Some shell providers return the absolute parsing path. Strip the
            // prefix so we hold a bare AUMID either way.
            if let Some(stripped) = aumid.strip_prefix("shell:AppsFolder\\") {
                aumid = stripped.to_string();
            }

            // AUMIDs contain '!' separating PackageFamilyName from AppId. Entries
            // without '!' are Win32 shortcuts the Start Menu walk already covers.
            if !aumid.contains('!') {
                continue;
            }

            if name.trim().is_empty() {
                continue;
            }

            out.push(UwpApp { title: name, aumid });
        }
    }
    out
}

unsafe fn get_display_name(item: &IShellItem, form: SIGDN) -> Option<String> {
    unsafe {
        let pwstr = item.GetDisplayName(form).ok()?;
        if pwstr.is_null() {
            return None;
        }
        let s = pwstr.to_string().ok();
        CoTaskMemFree(Some(pwstr.0 as _));
        s
    }
}
