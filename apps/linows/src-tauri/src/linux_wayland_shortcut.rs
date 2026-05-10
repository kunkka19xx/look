//! Wayland global shortcut via GNOME custom keybinding + D-Bus service.
//!
//! On Wayland, apps cannot grab global hotkeys directly (unlike X11).
//! The XDG GlobalShortcuts portal is not yet fully functional on GNOME 48.
//!
//! Instead, we:
//! 1. Register a D-Bus service (`com.look.Desktop`) that listens for `Toggle` calls
//! 2. Set up a GNOME custom keybinding (Alt+Space) that calls our D-Bus service
//!
//! This is the same approach used by other launchers (ulauncher, etc.) on GNOME Wayland.

use std::process::Command;

const DBUS_NAME: &str = "com.look.Desktop";
const DBUS_PATH: &str = "/com/look/Desktop";
const DBUS_IFACE: &str = "com.look.Desktop";

const KEYBINDING_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/look-toggle/";
const KEYBINDING_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";

/// Start a background thread that:
/// 1. Registers a D-Bus service to listen for Toggle calls
/// 2. Ensures a GNOME custom keybinding exists for Alt+Space
pub fn start<F>(on_toggle: F)
where
    F: Fn() + Send + Sync + 'static,
{
    ensure_gnome_keybinding();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for D-Bus service");

        if let Err(e) = rt.block_on(run_dbus_service(on_toggle)) {
            eprintln!("[look] D-Bus service error: {e}");
        }
    });
}

/// Register a GNOME custom keybinding for Alt+Space → dbus-send to our service.
fn ensure_gnome_keybinding() {
    // Check if our keybinding already exists
    let existing = gsettings_get(MEDIA_KEYS_SCHEMA, "custom-keybindings");
    if existing.contains(KEYBINDING_PATH) {
        // Already registered, verify the binding is still correct
        let current_binding = gsettings_get_at(KEYBINDING_SCHEMA, "binding", KEYBINDING_PATH);
        if current_binding.contains("<Alt>space") {
            return;
        }
    }

    // Set up the custom keybinding
    let toggle_cmd = format!(
        "dbus-send --session --type=method_call --dest={DBUS_NAME} {DBUS_PATH} {DBUS_IFACE}.Toggle"
    );

    gsettings_set_at(KEYBINDING_SCHEMA, "name", "'Look Toggle'", KEYBINDING_PATH);
    gsettings_set_at(
        KEYBINDING_SCHEMA,
        "command",
        &format!("'{toggle_cmd}'"),
        KEYBINDING_PATH,
    );
    gsettings_set_at(
        KEYBINDING_SCHEMA,
        "binding",
        "'<Alt>space'",
        KEYBINDING_PATH,
    );

    // Add our path to the custom-keybindings list
    let mut paths: Vec<String> = parse_gsettings_array(&existing);
    if !paths.iter().any(|p| p == KEYBINDING_PATH) {
        paths.push(KEYBINDING_PATH.to_string());
    }
    let new_value = format!(
        "[{}]",
        paths
            .iter()
            .map(|p| format!("'{p}'"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    gsettings_set(MEDIA_KEYS_SCHEMA, "custom-keybindings", &new_value);
    eprintln!("[look] Registered GNOME keybinding: Alt+Space → Look toggle");
}

/// Remove the GNOME custom keybinding registered by Look.
pub fn cleanup_gnome_keybinding() {
    let existing = gsettings_get(MEDIA_KEYS_SCHEMA, "custom-keybindings");
    let paths: Vec<String> = parse_gsettings_array(&existing)
        .into_iter()
        .filter(|p| p != KEYBINDING_PATH)
        .collect();
    let new_value = if paths.is_empty() {
        "@as []".to_string()
    } else {
        format!(
            "[{}]",
            paths
                .iter()
                .map(|p| format!("'{p}'"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    gsettings_set(MEDIA_KEYS_SCHEMA, "custom-keybindings", &new_value);
    eprintln!("[look] Removed GNOME keybinding for Alt+Space");
}

/// Run a D-Bus service that listens for Toggle method calls.
async fn run_dbus_service<F>(on_toggle: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn() + Send + Sync + 'static,
{
    struct LookService<F: Fn() + Send + Sync + 'static> {
        on_toggle: F,
    }

    #[zbus::interface(name = "com.look.Desktop")]
    impl<F: Fn() + Send + Sync + 'static> LookService<F> {
        fn toggle(&self) {
            (self.on_toggle)();
        }
    }

    let service = LookService { on_toggle };
    let _conn = zbus::connection::Builder::session()?
        .name(DBUS_NAME)?
        .serve_at(DBUS_PATH, service)?
        .build()
        .await?;

    eprintln!("[look] D-Bus service listening on {DBUS_NAME}");

    // Keep the service alive
    std::future::pending::<()>().await;
    Ok(())
}

// --- gsettings helpers ---

fn gsettings_get(schema: &str, key: &str) -> String {
    Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn gsettings_set(schema: &str, key: &str, value: &str) {
    let _ = Command::new("gsettings")
        .args(["set", schema, key, value])
        .output();
}

fn gsettings_get_at(schema: &str, key: &str, path: &str) -> String {
    Command::new("gsettings")
        .args(["get", &format!("{schema}:{path}"), key])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn gsettings_set_at(schema: &str, key: &str, value: &str, path: &str) {
    let _ = Command::new("gsettings")
        .args(["set", &format!("{schema}:{path}"), key, value])
        .output();
}

fn parse_gsettings_array(s: &str) -> Vec<String> {
    // Parse "@as []" or "['path1', 'path2']"
    let trimmed = s.trim();
    if trimmed == "@as []" || trimmed == "[]" {
        return Vec::new();
    }
    trimmed
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|p| p.trim().trim_matches('\'').trim_matches('"').to_string())
        .filter(|p| !p.is_empty())
        .collect()
}
