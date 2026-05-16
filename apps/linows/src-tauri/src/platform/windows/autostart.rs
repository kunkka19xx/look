//! Windows autostart stub. Real impl (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
//! lands in M3.

pub(crate) fn set(enabled: bool) -> Result<(), String> {
    // TODO(M3): write/remove HKCU\Software\Microsoft\Windows\CurrentVersion\Run entry.
    //
    // Until then, fail loudly when the user *enables* autostart so the UI
    // doesn't pretend the setting took effect (it wouldn't — the app would
    // silently not start at login). Succeed when disabling, because the actual
    // state (not registered) already matches what the user is asking for.
    if enabled {
        Err("Autostart on Windows is not yet implemented (M3)".to_string())
    } else {
        Ok(())
    }
}

pub(crate) fn get() -> bool {
    // TODO(M3): query HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
    false
}
