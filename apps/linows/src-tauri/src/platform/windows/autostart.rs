//! Windows autostart stub. Real impl (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
//! lands in M3.

pub(crate) fn set(_enabled: bool) -> Result<(), String> {
    // TODO(M3): write/remove HKCU\Software\Microsoft\Windows\CurrentVersion\Run entry.
    Ok(())
}

pub(crate) fn get() -> bool {
    // TODO(M3): query HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
    false
}
