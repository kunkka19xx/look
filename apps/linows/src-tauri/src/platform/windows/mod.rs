use std::sync::Once;
use windows::Win32::System::Com::CoIncrementMTAUsage;

/// Keep the process in an MTA so WinRT/COM calls on pooled blocking threads work:
/// a thread that never called `CoInitializeEx` joins the implicit MTA.
pub fn ensure_mta() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = unsafe { CoIncrementMTAUsage() };
    });
}

pub mod autostart;
pub mod clipboard;
pub mod drives;
pub mod effects;
pub mod fonts;
pub mod icons;
pub mod known_folders;
pub mod launch;
pub mod process;
pub mod recycle_bin;
pub mod sysinfo;
pub mod tools;
pub mod update;
pub mod version;
pub mod window_focus;
