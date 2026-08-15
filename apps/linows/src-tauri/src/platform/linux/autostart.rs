//! Linux autostart via XDG `.desktop` file in `$XDG_CONFIG_HOME/autostart/`.

use std::path::{Path, PathBuf};

fn autostart_dir() -> PathBuf {
    let config = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.config")
    });
    PathBuf::from(config).join("autostart")
}

fn desktop_entry_path() -> PathBuf {
    autostart_dir().join("look.desktop")
}

/// Base name behind a Nix wrapper file name (`.lookapp-wrapped` -> `lookapp`).
/// Nested wrappers append underscores (`.lookapp-wrapped_`).
fn wrapper_base_name(file_name: &str) -> Option<&str> {
    let (base, suffix) = file_name.strip_prefix('.')?.split_once("-wrapped")?;
    if base.is_empty() || suffix.chars().any(|c| c != '_') {
        return None;
    }
    Some(base)
}

/// Nix wraps GUI binaries: `bin/lookapp` sets up the environment and execs
/// `bin/.lookapp-wrapped`, which is what `current_exe()` reports. Starting the
/// inner binary skips that environment and leaves argv[0] as `.lookapp-wrapped`,
/// which GTK hands to the compositor as the app_id - GNOME then matches no
/// desktop entry and draws a placeholder icon instead of ours.
fn unwrap_launcher(exe: &Path) -> Option<PathBuf> {
    let base = wrapper_base_name(exe.file_name()?.to_str()?)?;
    let wrapper = exe.with_file_name(base);
    wrapper.exists().then_some(wrapper)
}

/// Prefer a `PATH` entry resolving to the same binary: profile paths survive
/// updates, while a pinned `/nix/store` path dies with its generation.
fn path_alias(exe: &Path) -> Option<PathBuf> {
    let name = exe.file_name()?;
    let target = exe.canonicalize().ok()?;
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.canonicalize().is_ok_and(|c| c == target))
}

fn current_exe_path() -> String {
    // Under an AppImage, current_exe() resolves to the temporary FUSE mount
    // (/tmp/.mount_Look_*/usr/bin/lookapp), gone by next login. The runtime
    // exposes the .AppImage path itself in $APPIMAGE - use that instead.
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        return appimage;
    }
    let Ok(exe) = std::env::current_exe() else {
        return "lookapp".to_string();
    };
    let exe = unwrap_launcher(&exe).unwrap_or(exe);
    path_alias(&exe)
        .unwrap_or(exe)
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn set(enabled: bool) -> Result<(), String> {
    let path = desktop_entry_path();
    if enabled {
        let dir = autostart_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create autostart dir: {e}"))?;
        let exe = current_exe_path();
        let contents = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Look\n\
             Exec={exe}\n\
             Icon=lookapp\n\
             Comment=Desktop launcher\n\
             X-GNOME-Autostart-enabled=true\n\
             StartupNotify=false\n"
        );
        std::fs::write(&path, contents).map_err(|e| format!("Failed to write autostart file: {e}"))
    } else if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to remove autostart file: {e}"))
    } else {
        Ok(())
    }
}

pub(crate) fn get() -> bool {
    desktop_entry_path().exists()
}

#[cfg(test)]
mod tests {
    use super::wrapper_base_name;

    #[test]
    fn strips_nix_wrapper_names() {
        assert_eq!(wrapper_base_name(".lookapp-wrapped"), Some("lookapp"));
        assert_eq!(wrapper_base_name(".lookapp-wrapped_"), Some("lookapp"));
        assert_eq!(wrapper_base_name("lookapp"), None);
        assert_eq!(wrapper_base_name(".lookapp"), None);
        assert_eq!(wrapper_base_name(".-wrapped"), None);
        assert_eq!(wrapper_base_name(".lookapp-wrapped.bak"), None);
    }
}
