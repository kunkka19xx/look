//! Wayland global shortcut via D-Bus service + compositor-specific keybinding.
//!
//! On Wayland, apps cannot grab global hotkeys directly (unlike X11).
//!
//! We:
//! 1. Register a D-Bus service (`com.look.Desktop`) that listens for `Toggle` calls
//! 2. Register a keybinding in the running compositor that calls our D-Bus service:
//!    - GNOME: custom keybinding via gsettings
//!    - KDE: kglobalaccel D-Bus registration (signal-driven, no command hop)
//!    - Sway: `swaymsg bindsym ...`
//!    - Hyprland: `hyprctl keyword bind ...`

use super::host_command;
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
    Kde,
    Sway,
    Hyprland,
    Other,
}

fn detect_compositor() -> Compositor {
    if super::wm::is_sway() {
        return Compositor::Sway;
    }
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return Compositor::Hyprland;
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let desktop_is = |name: &str| {
        desktop
            .split(':')
            .any(|s| s.trim().eq_ignore_ascii_case(name))
    };
    if desktop_is("GNOME") {
        return Compositor::Gnome;
    }
    if desktop_is("KDE") {
        return Compositor::Kde;
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

    std::thread::spawn(move || {
        // Registration runs off the main thread: it shells out to
        // gsettings/swaymsg/hyprctl and the GNOME path sleeps between writes.
        match compositor {
            Compositor::Gnome => ensure_gnome_keybinding(),
            Compositor::Sway => ensure_sway_keybinding(),
            Compositor::Hyprland => ensure_hyprland_keybinding(),
            // KDE registers via async D-Bus alongside the Toggle service below.
            Compositor::Kde => {}
            Compositor::Other => {
                eprintln!(
                    "[look] Unknown Wayland compositor: Alt+Space must be bound manually.\n\
                     [look] Bind your hotkey to run: {TOGGLE_CMD}"
                );
            }
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for D-Bus service");

        rt.block_on(async {
            let on_toggle = std::sync::Arc::new(on_toggle);

            if compositor == Compositor::Kde {
                let toggle = on_toggle.clone();
                tokio::task::spawn(async move {
                    if let Err(e) = run_kde_keybinding(move || toggle()).await {
                        eprintln!(
                            "[look] KDE keybinding error: {e}\n\
                             [look] Bind your hotkey manually to run: {TOGGLE_CMD}"
                        );
                    }
                });
            }

            if let Err(e) = run_dbus_service(move || on_toggle()).await {
                eprintln!("[look] D-Bus service error: {e}");
                // The KDE task toggles via the kglobalaccel signal alone;
                // keep it alive.
                if compositor == Compositor::Kde {
                    std::future::pending::<()>().await;
                }
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Sway
// ---------------------------------------------------------------------------

fn ensure_sway_keybinding() {
    // Add window rule: float + no border
    let _ = host_command("swaymsg")
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
    let _ = host_command("swaymsg")
        .arg(format!("bindsym Alt+space exec {TOGGLE_CMD}"))
        .output();

    eprintln!("[look] Registered Sway keybinding: Alt+Space → Look toggle");
}

fn cleanup_sway_keybinding() {
    let _ = host_command("swaymsg").arg("unbindsym Alt+space").output();
    eprintln!("[look] Removed Sway keybinding for Alt+Space");
}

// ---------------------------------------------------------------------------
// Hyprland
// ---------------------------------------------------------------------------

fn ensure_hyprland_keybinding() {
    // Hyprland v0.55+ uses Lua config - `hyprctl eval` with hl.* API.
    // Older versions use `hyprctl keyword bind ...` (INI-style parser).
    //
    // hl.bind stacks duplicates on every call (hot-reloads in dev, or
    // sequential launches in prod), so unbind first via pcall - pcall keeps
    // the eval succeeding even when the binding doesn't exist yet (first run).
    let lua = format!(
        r#"pcall(hl.unbind, "ALT + space")
hl.window_rule({{ name = "look-float", match = {{ class = "lookapp" }}, float = true }})
hl.window_rule({{ name = "look-noborder", match = {{ class = "lookapp" }}, border_size = 0, rounding = 0, no_shadow = true }})
hl.bind("ALT + space", hl.dsp.exec_cmd("{TOGGLE_CMD}"))"#
    );

    let result = host_command("hyprctl").args(["eval", &lua]).output();

    let used_lua = result
        .as_ref()
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("error"))
        .unwrap_or(false);

    if !used_lua {
        // Fallback: legacy keyword syntax for older Hyprland
        let _ = host_command("hyprctl")
            .args(["keyword", "windowrulev2", "float, class:lookapp"])
            .output();
        let _ = host_command("hyprctl")
            .args(["keyword", "windowrulev2", "noborder, class:lookapp"])
            .output();
        let _ = host_command("hyprctl")
            .args(["keyword", "bind", &format!("ALT,space,exec,{TOGGLE_CMD}")])
            .output();
    }

    eprintln!("[look] Registered Hyprland keybinding: Alt+Space → Look toggle");
}

fn cleanup_hyprland_keybinding() {
    // Try Lua first, then legacy
    let result = host_command("hyprctl")
        .args(["eval", r#"hl.unbind("ALT + space")"#])
        .output();

    let used_lua = result.as_ref().map(|o| o.status.success()).unwrap_or(false);

    if !used_lua {
        let _ = host_command("hyprctl")
            .args(["keyword", "unbind", "ALT,space"])
            .output();
    }

    eprintln!("[look] Removed Hyprland keybinding for Alt+Space");
}

// ---------------------------------------------------------------------------
// GNOME
// ---------------------------------------------------------------------------

/// Register a GNOME custom keybinding for Alt+Space → dbus-send to our service.
///
/// Order matters: mutter refuses gsd's grab while activate-window-menu holds
/// Alt+Space and gsd never retries a failed grab, so the key must be freed
/// before the binding is written or Alt+Space stays dead until re-login.
fn ensure_gnome_keybinding() {
    let had_conflict = disable_window_menu_binding();
    if had_conflict {
        // Give mutter a moment to process the release before gsd re-grabs.
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let existing = gsettings_get(MEDIA_KEYS_SCHEMA, "custom-keybindings");
    let already_bound = existing.contains(KEYBINDING_PATH)
        && gsettings_get_at(KEYBINDING_SCHEMA, "binding", KEYBINDING_PATH).contains("<Alt>space");

    // Registered by a previous run and nothing shadowed it: the grab is live.
    if already_bound && !had_conflict {
        return;
    }

    gsettings_set_at(KEYBINDING_SCHEMA, "name", "'Look Toggle'", KEYBINDING_PATH);
    gsettings_set_at(
        KEYBINDING_SCHEMA,
        "command",
        &format!("'{TOGGLE_CMD}'"),
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

    // Write the binding last, toggling it when it was shadowed: dconf drops
    // same-value writes and gsd only re-grabs on a change notification.
    if already_bound {
        gsettings_set_at(KEYBINDING_SCHEMA, "binding", "''", KEYBINDING_PATH);
    }
    gsettings_set_at(
        KEYBINDING_SCHEMA,
        "binding",
        "'<Alt>space'",
        KEYBINDING_PATH,
    );

    eprintln!("[look] Registered GNOME keybinding: Alt+Space → Look toggle");
}

/// Disable GNOME's default Alt+Space (window menu) if it holds the key,
/// saving the original for restore on exit. Returns true when cleared.
fn disable_window_menu_binding() -> bool {
    let wm_binding = gsettings_get("org.gnome.desktop.wm.keybindings", "activate-window-menu");
    if !wm_binding.contains("<Alt>space") {
        return false;
    }
    if let Ok(mut saved) = SAVED_WM_BINDING.lock() {
        *saved = Some(wm_binding);
    }
    gsettings_set(
        "org.gnome.desktop.wm.keybindings",
        "activate-window-menu",
        "['']",
    );
    eprintln!("[look] Disabled GNOME default Alt+Space (window menu) to avoid conflict");
    true
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
// KDE (kglobalaccel)
// ---------------------------------------------------------------------------

const KGA_BUS: &str = "org.kde.kglobalaccel";
const KGA_PATH: &str = "/kglobalaccel";
const KGA_IFACE: &str = "org.kde.KGlobalAccel";
/// Object path derived from our component unique name ("lookapp").
const KGA_COMPONENT_PATH: &str = "/component/lookapp";
const KGA_COMPONENT_IFACE: &str = "org.kde.kglobalaccel.Component";

/// QKeySequence int encoding of Alt+Space (Qt::AltModifier | Qt::Key_Space).
const QT_ALT_SPACE: i32 = 0x0800_0020;
/// kglobalaccel SetShortcutFlag values (kglobalaccel_p.h).
const KGA_SET_PRESENT: u32 = 2;
const KGA_IS_DEFAULT: u32 = 8;

/// KRunner actions that hold Alt+Space by default; the "krunner" pair
/// covers pre-service Plasma 5.
const KRUNNER_ACTIONS: &[(&str, &str)] = &[
    ("org.kde.krunner.desktop", "_launch"),
    ("krunner", "run command"),
];

/// KRunner bindings Look cleared to free Alt+Space, restored on exit.
static SAVED_KRUNNER_KEYS: Mutex<Vec<(Vec<String>, Vec<i32>)>> = Mutex::new(Vec::new());

/// kglobalaccel actionId: [component unique, action unique, friendly component, friendly action].
fn look_action_id() -> Vec<String> {
    ["lookapp", "toggle", "Look", "Toggle Look"]
        .map(String::from)
        .into()
}

fn krunner_action_id(component: &str, action: &str) -> Vec<String> {
    vec![
        component.into(),
        action.into(),
        String::new(),
        String::new(),
    ]
}

/// Register Alt+Space with kglobalaccel and toggle on its
/// `globalShortcutPressed` signal. The KF5-era int-key methods are still
/// served by the KF6 daemon, so one path covers Plasma 5 and 6.
async fn run_kde_keybinding<F>(on_toggle: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn() + Send + Sync + 'static,
{
    use futures_util::StreamExt;

    let conn = zbus::Connection::session().await?;
    let kga = zbus::Proxy::new(&conn, KGA_BUS, KGA_PATH, KGA_IFACE).await?;
    let action_id = look_action_id();

    kga.call_method("doRegister", &(&action_id,)).await?;

    // kglobalaccel drops clashing keys rather than stealing them: free
    // Alt+Space from KRunner first, saving its binding for restore on exit.
    for (component, action) in KRUNNER_ACTIONS {
        let id = krunner_action_id(component, action);
        let Ok(keys) = kga.call::<_, _, Vec<i32>>("shortcut", &(&id,)).await else {
            continue;
        };
        if !keys.contains(&QT_ALT_SPACE) {
            continue;
        }
        let remaining: Vec<i32> = keys
            .iter()
            .copied()
            .filter(|&k| k != QT_ALT_SPACE)
            .collect();
        if kga
            .call_method("setForeignShortcut", &(&id, &remaining))
            .await
            .is_ok()
        {
            if let Ok(mut saved) = SAVED_KRUNNER_KEYS.lock() {
                saved.push((id, keys));
            }
            eprintln!("[look] Freed Alt+Space from KRunner ({component}) to avoid conflict");
        }
    }

    // IsDefault shows Alt+Space as the default in System Settings; SetPresent
    // without NoAutoloading lets a user rebind survive restarts.
    let _: Vec<i32> = kga
        .call(
            "setShortcut",
            &(&action_id, &vec![QT_ALT_SPACE], KGA_IS_DEFAULT),
        )
        .await
        .unwrap_or_default();
    let granted: Vec<i32> = kga
        .call(
            "setShortcut",
            &(&action_id, &vec![QT_ALT_SPACE], KGA_SET_PRESENT),
        )
        .await?;

    if granted.contains(&QT_ALT_SPACE) {
        eprintln!("[look] Registered KDE keybinding: Alt+Space → Look toggle");
    } else {
        eprintln!(
            "[look] KDE assigned {granted:?} instead of Alt+Space. Rebind it in \
             System Settings → Shortcuts → Look, or bind a key to run: {TOGGLE_CMD}"
        );
    }

    let component =
        zbus::Proxy::new(&conn, KGA_BUS, KGA_COMPONENT_PATH, KGA_COMPONENT_IFACE).await?;
    let mut presses = component.receive_signal("globalShortcutPressed").await?;
    while let Some(msg) = presses.next().await {
        let Ok((_, action, _)) = msg.body().deserialize::<(String, String, i64)>() else {
            continue;
        };
        if action == "toggle" {
            on_toggle();
        }
    }
    Ok(())
}

/// Unregister Look's kglobalaccel action and restore KRunner's Alt+Space.
fn cleanup_kde_keybinding() {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    rt.block_on(async {
        let Ok(conn) = zbus::Connection::session().await else {
            return;
        };
        let Ok(kga) = zbus::Proxy::new(&conn, KGA_BUS, KGA_PATH, KGA_IFACE).await else {
            return;
        };
        let _ = kga.call_method("unRegister", &(&look_action_id(),)).await;
        let saved: Vec<_> = SAVED_KRUNNER_KEYS
            .lock()
            .map(|mut s| std::mem::take(&mut *s))
            .unwrap_or_default();
        for (id, keys) in saved {
            let _ = kga.call_method("setForeignShortcut", &(&id, &keys)).await;
        }
    });
    eprintln!("[look] Removed KDE keybinding for Alt+Space");
}

// ---------------------------------------------------------------------------
// Cleanup dispatcher (called on exit from main.rs)
// ---------------------------------------------------------------------------

pub fn cleanup_keybinding() {
    match detect_compositor() {
        Compositor::Gnome => cleanup_gnome_keybinding(),
        Compositor::Kde => cleanup_kde_keybinding(),
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
    host_command("gsettings")
        .args(["get", schema, key])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn gsettings_set(schema: &str, key: &str, value: &str) {
    let _ = host_command("gsettings")
        .args(["set", schema, key, value])
        .output();
}

fn gsettings_get_at(schema: &str, key: &str, path: &str) -> String {
    host_command("gsettings")
        .args(["get", &format!("{schema}:{path}"), key])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn gsettings_set_at(schema: &str, key: &str, value: &str, path: &str) {
    let _ = host_command("gsettings")
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
