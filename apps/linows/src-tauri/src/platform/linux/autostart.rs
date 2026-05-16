//! Linux autostart via XDG `.desktop` file in `$XDG_CONFIG_HOME/autostart/`.

use std::path::PathBuf;

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

fn current_exe_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "lookapp".to_string())
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
             Icon=look\n\
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
