//! "Add lookapp to PATH". Windows-only work: the Linux packages already
//! drop the binary somewhere on PATH, and macOS symlinks it at install time.

/// Debug builds never add themselves: the entry would be `target\debug`.
/// Removing still goes through.
pub fn set_cli_path(enabled: bool) -> Result<(), String> {
    if enabled && cfg!(debug_assertions) {
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::path_env::set(enabled)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(())
    }
}
pub fn get_cli_path() -> bool {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::path_env::get()
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}
