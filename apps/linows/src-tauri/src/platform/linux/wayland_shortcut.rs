//! Wayland global shortcut via D-Bus service + compositor-specific keybinding.
//!
//! On Wayland, apps cannot grab global hotkeys directly (unlike X11).
//!
//! We:
//! 1. Register a D-Bus service (`com.look.Desktop`) that listens for `Toggle` calls
//! 2. Register a keybinding in the running compositor that calls our D-Bus service:
//!    - GNOME: custom keybinding via gsettings
//!    - Sway: `swaymsg bindsym ...`
//!    - Hyprland: `hyprctl keyword bind ...`

use std::process::Command;
use std::sync::Mutex;

/// Saved original value of activate-window-menu before Look disabled it.
static SAVED_WM_BINDING: Mutex<Option<String>> = Mutex::new(None);

const DBUS_NAME: &str = "com.look.Desktop";
const DBUS_PATH: &str = "/com/look/Desktop";
const KEYBINDING_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/look-toggle/";
const KEYBINDING_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";

const TOGGLE_CMD: &str = concat!(
    "dbus-send --session --type=method_call",
    " --dest=com.look.Desktop /com/look/Desktop com.look.Desktop.Toggle"
);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Compositor {
    Gnome,
    Sway,
    Hyprland,
    Other,
}

fn detect_compositor() -> Compositor {
    if std::env::var("SWAYSOCK").is_ok() {
        return Compositor::Sway;
    }
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return Compositor::Hyprland;
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if desktop
        .split(':')
        .any(|s| s.trim().eq_ignore_ascii_case("GNOME"))
    {
        return Compositor::Gnome;
    }
    Compositor::Other
}

/// Start a background thread that:
/// 1. Registers a compositor-specific keybinding for Alt+Space
/// 2. Registers a D-Bus service to listen for Toggle calls
pub fn start<F>(on_toggle: F)
where
    F: Fn() + Send + Sync + 'static,
{
    let compositor = detect_compositor();
    match compositor {
        Compositor::Gnome => ensure_gnome_keybinding(),
        Compositor::Sway => ensure_sway_keybinding(),
        Compositor::Hyprland => ensure_hyprland_keybinding(),
        Compositor::Other => {
            eprintln!(
                "[look] Unknown Wayland compositor — Alt+Space must be bound manually.\n\
                 [look] Bind your hotkey to run: {TOGGLE_CMD}"
            );
        }
    }

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

// ---------------------------------------------------------------------------
// Sway
// ---------------------------------------------------------------------------

fn ensure_sway_keybinding() {
    // Add window rule: float + no border
    let _ = Command::new("swaymsg")
        .args([
            "for_window",
            "[app_id=\"lookapp\"]",
            "floating",
            "enable,",
            "border",
            "none",
        ])
        .output();

    // Bind Alt+Space to toggle Look via D-Bus
    let _ = Command::new("swaymsg")
        .arg(format!("bindsym Alt+space exec {TOGGLE_CMD}"))
        .output();

    eprintln!("[look] Registered Sway keybinding: Alt+Space → Look toggle");
}

fn cleanup_sway_keybinding() {
    let _ = Command::new("swaymsg").arg("unbindsym Alt+space").output();
    eprintln!("[look] Removed Sway keybinding for Alt+Space");
}

// ---------------------------------------------------------------------------
// Hyprland
// ---------------------------------------------------------------------------

fn ensure_hyprland_keybinding() {
    // Hyprland v0.55+ uses Lua config — `hyprctl eval` with hl.* API.
    // Older versions use `hyprctl keyword bind ...` (INI-style parser).
    //
    // hl.bind stacks duplicates on every call (hot-reloads in dev, or
    // sequential launches in prod), so unbind first via pcall — pcall keeps
    // the eval succeeding even when the binding doesn't exist yet (first run).
    let lua = format!(
        r#"pcall(hl.unbind, "ALT + space")
hl.window_rule({{ name = "look-float", match = {{ class = "lookapp" }}, float = true }})
hl.window_rule({{ name = "look-noborder", match = {{ class = "lookapp" }}, border_size = 0, rounding = 0, no_shadow = true }})
hl.bind("ALT + space", hl.dsp.exec_cmd("{TOGGLE_CMD}"))"#
    );

    let result = Command::new("hyprctl").args(["eval", &lua]).output();

    let used_lua = result
        .as_ref()
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("error"))
        .unwrap_or(false);

    if !used_lua {
        // Fallback: legacy keyword syntax for older Hyprland
        let _ = Command::new("hyprctl")
            .args(["keyword", "windowrulev2", "float, class:lookapp"])
            .output();
        let _ = Command::new("hyprctl")
            .args(["keyword", "windowrulev2", "noborder, class:lookapp"])
            .output();
        let _ = Command::new("hyprctl")
            .args(["keyword", "bind", &format!("ALT,space,exec,{TOGGLE_CMD}")])
            .output();
    }

    eprintln!("[look] Registered Hyprland keybinding: Alt+Space → Look toggle");
}

fn cleanup_hyprland_keybinding() {
    // Try Lua first, then legacy
    let result = Command::new("hyprctl")
        .args(["eval", r#"hl.unbind("ALT + space")"#])
        .output();

    let used_lua = result.as_ref().map(|o| o.status.success()).unwrap_or(false);

    if !used_lua {
        let _ = Command::new("hyprctl")
            .args(["keyword", "unbind", "ALT,space"])
            .output();
    }

    eprintln!("[look] Removed Hyprland keybinding for Alt+Space");
}

// ---------------------------------------------------------------------------
// GNOME
// ---------------------------------------------------------------------------

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
    gsettings_set_at(KEYBINDING_SCHEMA, "name", "'Look Toggle'", KEYBINDING_PATH);
    gsettings_set_at(
        KEYBINDING_SCHEMA,
        "command",
        &format!("'{TOGGLE_CMD}'"),
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

    // Disable GNOME's default Alt+Space (window menu) so it doesn't shadow ours.
    // Save the original value so we can restore it on exit.
    let wm_binding = gsettings_get("org.gnome.desktop.wm.keybindings", "activate-window-menu");
    if wm_binding.contains("<Alt>space") {
        if let Ok(mut saved) = SAVED_WM_BINDING.lock() {
            *saved = Some(wm_binding);
        }
        gsettings_set(
            "org.gnome.desktop.wm.keybindings",
            "activate-window-menu",
            "['']",
        );
        eprintln!("[look] Disabled GNOME default Alt+Space (window menu) to avoid conflict");
    }

    eprintln!("[look] Registered GNOME keybinding: Alt+Space → Look toggle");
}

/// Remove the GNOME custom keybinding registered by Look.
fn cleanup_gnome_keybinding() {
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

    // Restore the original activate-window-menu binding
    let original = SAVED_WM_BINDING.lock().ok().and_then(|guard| guard.clone());
    if let Some(val) = original {
        gsettings_set(
            "org.gnome.desktop.wm.keybindings",
            "activate-window-menu",
            &val,
        );
    }

    eprintln!("[look] Removed GNOME keybinding for Alt+Space");
}

// ---------------------------------------------------------------------------
// Cleanup dispatcher (called on exit from main.rs)
// ---------------------------------------------------------------------------

pub fn cleanup_keybinding() {
    match detect_compositor() {
        Compositor::Gnome => cleanup_gnome_keybinding(),
        Compositor::Sway => cleanup_sway_keybinding(),
        Compositor::Hyprland => cleanup_hyprland_keybinding(),
        Compositor::Other => {}
    }
}

// ---------------------------------------------------------------------------
// D-Bus service
// ---------------------------------------------------------------------------

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
