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
/// Relative entries (`.`, or the empty entry `PATH=/usr/bin:`) are skipped:
/// `Exec` takes an absolute path or a bare name resolved against `PATH`.
fn path_alias(exe: &Path) -> Option<PathBuf> {
    let name = exe.file_name()?;
    let target = exe.canonicalize().ok()?;
    std::env::split_paths(&std::env::var_os("PATH")?)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.canonicalize().is_ok_and(|c| c == target))
}

/// Characters that force quoting of an `Exec` argument, per the Desktop Entry
/// spec. Space is the one that shows up in practice (`~/My Apps/Look.AppImage`).
const EXEC_RESERVED: &[char] = &[
    ' ', '\t', '\n', '\r', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(',
    ')', '`',
];

/// Serialize a path as an `Exec` argument. Escaping is two-layered: the Exec
/// grammar backslash-escapes `"`, backtick, `$` and `\` inside quotes, then the
/// key file's string type escapes each backslash again - so a literal backslash
/// ends up as four, and control characters as `\n`-style sequences. `%` doubles
/// regardless of quoting, else `100%foo` reads as the `%f` field code.
fn exec_arg(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if !raw.contains(EXEC_RESERVED) {
        return raw.replace('%', "%%");
    }
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for c in raw.chars() {
        match c {
            '%' => out.push_str("%%"),
            '\\' => out.push_str(r"\\\\"),
            '"' | '`' | '$' => {
                out.push_str(r"\\");
                out.push(c);
            }
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn current_exe_path() -> PathBuf {
    // Under an AppImage, current_exe() resolves to the temporary FUSE mount
    // (/tmp/.mount_Look_*/usr/bin/lookapp), gone by next login. The runtime
    // exposes the .AppImage path itself in $APPIMAGE - use that instead.
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let appimage = PathBuf::from(appimage);
        return std::path::absolute(&appimage).unwrap_or(appimage);
    }
    let Ok(exe) = std::env::current_exe() else {
        return PathBuf::from("lookapp");
    };
    let exe = unwrap_launcher(&exe).unwrap_or(exe);
    path_alias(&exe).unwrap_or(exe)
}

pub(crate) fn set(enabled: bool) -> Result<(), String> {
    let path = desktop_entry_path();
    if enabled {
        let dir = autostart_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create autostart dir: {e}"))?;
        let exe = exec_arg(&current_exe_path());
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
    use super::{exec_arg, wrapper_base_name};
    use std::path::Path;

    #[test]
    fn leaves_plain_paths_unquoted() {
        assert_eq!(
            exec_arg(Path::new("/home/u/.nix-profile/bin/lookapp")),
            "/home/u/.nix-profile/bin/lookapp"
        );
    }

    #[test]
    fn quotes_paths_with_reserved_chars() {
        assert_eq!(
            exec_arg(Path::new("/home/u/My Apps/Look.AppImage")),
            "\"/home/u/My Apps/Look.AppImage\""
        );
        assert_eq!(
            exec_arg(Path::new("/opt/a$b/lookapp")),
            r#""/opt/a\\$b/lookapp""#
        );
        assert_eq!(
            exec_arg(Path::new(r"/opt/a\b/lookapp")),
            r#""/opt/a\\\\b/lookapp""#
        );
        assert_eq!(
            exec_arg(Path::new("/opt/a\"b/lookapp")),
            r#""/opt/a\\"b/lookapp""#
        );
    }

    #[test]
    fn doubles_percent_in_either_form() {
        assert_eq!(
            exec_arg(Path::new("/opt/100%foo/lookapp")),
            "/opt/100%%foo/lookapp"
        );
        assert_eq!(
            exec_arg(Path::new("/opt/100%foo bar/lookapp")),
            "\"/opt/100%%foo bar/lookapp\""
        );
    }

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
