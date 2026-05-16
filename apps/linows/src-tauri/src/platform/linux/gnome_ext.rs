//! Auto-install and communicate with the Look GNOME Shell extension.
//!
//! The extension exposes a single D-Bus method `FocusApp(desktop_id)` that
//! uses gnome-shell's internal `Shell.App.activate()` to focus existing
//! windows — the same mechanism GNOME's own Activities search uses.

use std::path::PathBuf;

const EXT_UUID: &str = "look-integration@lookapp";
const DBUS_NAME: &str = "com.look.ShellIntegration";
const DBUS_PATH: &str = "/com/look/ShellIntegration";
const DBUS_IFACE: &str = "com.look.ShellIntegration";

const METADATA_JSON: &str = include_str!("../../gnome-shell-extension/metadata.json");
const EXTENSION_JS: &str = include_str!("../../gnome-shell-extension/extension.js");

/// Install the GNOME Shell extension if not already present, then enable it.
pub fn ensure_installed() {
    let ext_dir = extension_dir();
    let metadata_path = ext_dir.join("metadata.json");
    let extension_path = ext_dir.join("extension.js");

    // Check if already installed with current version
    let needs_install = if metadata_path.exists() && extension_path.exists() {
        let existing_ext = std::fs::read_to_string(&extension_path).unwrap_or_default();
        let existing_meta = std::fs::read_to_string(&metadata_path).unwrap_or_default();
        existing_ext != EXTENSION_JS || existing_meta != METADATA_JSON
    } else {
        true
    };

    if needs_install {
        // Use `gnome-extensions install` with a zip so GNOME Shell picks up
        // the extension immediately — no re-login required on Wayland.
        if install_via_gnome_extensions() {
            eprintln!("[look] Installed GNOME Shell extension via gnome-extensions install");
            enable_extension();
            return;
        }

        // Fallback: write files manually (needs re-login on Wayland)
        if let Err(e) = std::fs::create_dir_all(&ext_dir) {
            eprintln!("[look] Failed to create extension dir: {e}");
            return;
        }
        if let Err(e) = std::fs::write(&metadata_path, METADATA_JSON) {
            eprintln!("[look] Failed to write metadata.json: {e}");
            return;
        }
        if let Err(e) = std::fs::write(&extension_path, EXTENSION_JS) {
            eprintln!("[look] Failed to write extension.js: {e}");
            return;
        }
        eprintln!("[look] Installed GNOME Shell extension: {EXT_UUID} (manual, needs re-login)");
        enable_extension();
    }
}

/// Build a zip of the extension in /tmp and install via `gnome-extensions install --force`.
fn install_via_gnome_extensions() -> bool {
    let zip_path = std::env::temp_dir().join("look-gnome-ext.zip");
    if let Err(e) = build_extension_zip(&zip_path) {
        eprintln!("[look] Failed to create extension zip: {e}");
        return false;
    }
    let result = std::process::Command::new("gnome-extensions")
        .args(["install", "--force"])
        .arg(&zip_path)
        .output();
    let _ = std::fs::remove_file(&zip_path);
    match result {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("[look] gnome-extensions install failed: {stderr}");
            false
        }
        Err(e) => {
            eprintln!("[look] gnome-extensions not found: {e}");
            false
        }
    }
}

fn build_extension_zip(path: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;

    let file = std::fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("metadata.json", options)?;
    zip.write_all(METADATA_JSON.as_bytes())?;

    zip.start_file("extension.js", options)?;
    zip.write_all(EXTENSION_JS.as_bytes())?;

    zip.finish()?;
    Ok(())
}

/// Try to focus an app by its desktop file ID using the GNOME Shell extension.
/// Returns true if the app was focused (had existing windows).
///
/// Uses zbus to call directly from Look's process (which has focus),
/// so GNOME trusts the activation request without showing a "ready" popup.
pub fn try_focus_app(desktop_id: &str) -> bool {
    // Build the tokio runtime inline — this is called from a sync context
    // and needs to complete quickly.
    let id = desktop_id.to_string();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(rt) = rt else { return false };

    rt.block_on(async {
        let conn = zbus::Connection::session().await.ok()?;
        let reply: (bool,) = conn
            .call_method(
                Some(DBUS_NAME),
                DBUS_PATH,
                Some(DBUS_IFACE),
                "FocusApp",
                &id,
            )
            .await
            .ok()?
            .body()
            .deserialize()
            .ok()?;
        Some(reply.0)
    })
    .unwrap_or(false)
}

fn extension_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("gnome-shell/extensions")
        .join(EXT_UUID)
}

fn enable_extension() {
    // Use gnome-extensions CLI which properly handles enabled/disabled state.
    // Raw gsettings manipulation misses the disabled-extensions list and
    // other state that GNOME tracks internally.
    let result = std::process::Command::new("gnome-extensions")
        .args(["enable", EXT_UUID])
        .output();

    match &result {
        Ok(output) if output.status.success() => {
            eprintln!("[look] Enabled extension via gnome-extensions CLI");
        }
        _ => {
            // Fallback: raw gsettings for older GNOME or minimal installs
            let output = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.shell", "enabled-extensions"])
                .output();

            let current = output
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .unwrap_or_default();
            let current = current.trim();

            if current.contains(EXT_UUID) {
                return;
            }

            let mut extensions: Vec<String> = if current == "@as []" || current == "[]" {
                Vec::new()
            } else {
                current
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(|s| s.trim().trim_matches('\'').trim_matches('"').to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };

            extensions.push(EXT_UUID.to_string());

            let new_value = format!(
                "[{}]",
                extensions
                    .iter()
                    .map(|e| format!("'{e}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            let _ = std::process::Command::new("gsettings")
                .args(["set", "org.gnome.shell", "enabled-extensions", &new_value])
                .output();
        }
    }
}
