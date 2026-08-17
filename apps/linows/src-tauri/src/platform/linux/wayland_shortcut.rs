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

use super::{host_binary_path, host_command};
use crate::health;
use std::sync::{Mutex, OnceLock};

/// Saved original value of activate-window-menu before Look disabled it.
static SAVED_WM_BINDING: Mutex<Option<String>> = Mutex::new(None);

const DBUS_NAME: &str = "com.look.Desktop";
const DBUS_PATH: &str = "/com/look/Desktop";
const KEYBINDING_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/look-toggle/";
const KEYBINDING_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";

const TOGGLE_METHOD: &str = "Toggle";

/// Command-line D-Bus callers able to invoke our Toggle method, most widely
/// installed first. `dbus-send` belongs to the D-Bus reference implementation,
/// not to the protocol: distros moving to dbus-broker split it into a
/// utilities package nothing depends on, so it is absent on Fedora and its
/// derivatives. `gdbus` comes from glib2, the same package as `gsettings`;
/// `busctl` comes from systemd.
const CALLER_GDBUS: &str = "gdbus";
const CALLER_DBUS_SEND: &str = "dbus-send";
const CALLER_BUSCTL: &str = "busctl";
const DBUS_CALLERS: &[&str] = &[CALLER_GDBUS, CALLER_DBUS_SEND, CALLER_BUSCTL];

/// Binaries here are spelled absolutely in the keybinding: on NixOS the
/// compositor that runs it does not share our `PATH`, but a store path
/// resolves identically from every process.
const NIX_STORE_PREFIX: &str = "/nix/store/";

/// A D-Bus caller found on this system.
struct DbusCaller {
    /// Which of `DBUS_CALLERS` it is, deciding the argument syntax.
    name: &'static str,
    /// How to spell it in a keybinding another process will run.
    invocation: String,
}

fn dbus_caller() -> Option<&'static DbusCaller> {
    static CALLER: OnceLock<Option<DbusCaller>> = OnceLock::new();
    CALLER
        .get_or_init(|| {
            DBUS_CALLERS.iter().find_map(|&name| {
                let path = host_binary_path(name)?;
                Some(DbusCaller {
                    name,
                    invocation: caller_invocation(name, &path),
                })
            })
        })
        .as_ref()
}

fn caller_invocation(name: &str, path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    if path.starts_with(NIX_STORE_PREFIX) {
        path.into_owned()
    } else {
        name.to_string()
    }
}

/// The command a compositor keybinding runs to toggle Look. Resolved once:
/// it gets written into compositor config that outlives this call.
fn toggle_cmd() -> &'static str {
    static CMD: OnceLock<String> = OnceLock::new();
    CMD.get_or_init(|| match dbus_caller() {
        Some(caller) => toggle_command(caller.name, &caller.invocation),
        None => toggle_command(CALLER_GDBUS, CALLER_GDBUS),
    })
}

fn toggle_command(name: &str, invocation: &str) -> String {
    match name {
        CALLER_DBUS_SEND => format!(
            "{invocation} --session --type=method_call --dest={DBUS_NAME} \
             {DBUS_PATH} {DBUS_NAME}.{TOGGLE_METHOD}"
        ),
        CALLER_BUSCTL => {
            format!("{invocation} --user call {DBUS_NAME} {DBUS_PATH} {DBUS_NAME} {TOGGLE_METHOD}")
        }
        _ => format!(
            "{invocation} call --session --dest {DBUS_NAME} --object-path {DBUS_PATH} \
             --method {DBUS_NAME}.{TOGGLE_METHOD}"
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Compositor {
    Gnome,
    Kde,
    Sway,
    Hyprland,
    Niri,
    Other,
}

fn detect_compositor() -> Compositor {
    if super::wm::is_sway() {
        return Compositor::Sway;
    }
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return Compositor::Hyprland;
    }
    if super::wm::is_niri() {
        return Compositor::Niri;
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
        // Reported before registration: health::report keeps the first message
        // per issue id, and a missing caller explains the dead key better than
        // the per-compositor failures that follow.
        if compositor != Compositor::Kde && dbus_caller().is_none() {
            health::report(
                health::ISSUE_HOTKEY,
                format!(
                    "Alt+Space cannot reach Look: none of these D-Bus \
                     command-line tools are installed ({}). Install one of \
                     them (gdbus comes with glib2) and restart Look.",
                    DBUS_CALLERS.join(", ")
                ),
            );
        }

        // Registration runs off the main thread: it shells out to
        // gsettings/swaymsg/hyprctl and the GNOME path sleeps between writes.
        match compositor {
            Compositor::Gnome => ensure_gnome_keybinding(),
            Compositor::Sway => ensure_sway_keybinding(),
            Compositor::Hyprland => ensure_hyprland_keybinding(),
            Compositor::Niri => report_niri_keybinding(),
            // KDE registers via async D-Bus alongside the Toggle service below.
            Compositor::Kde => {}
            Compositor::Other => {
                let cmd = toggle_cmd();
                health::report(
                    health::ISSUE_HOTKEY,
                    format!(
                        "This compositor has no supported hotkey API, so Alt+Space \
                         is not set up. Bind a key manually to run: {cmd}"
                    ),
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
                        let cmd = toggle_cmd();
                        health::report(
                            health::ISSUE_HOTKEY,
                            format!(
                                "KDE hotkey registration failed ({e}). Bind a key \
                                 manually to run: {cmd}"
                            ),
                        );
                    }
                });
            }

            if let Err(e) = run_dbus_service(move || on_toggle()).await {
                if compositor == Compositor::Kde {
                    // The KDE task toggles via the kglobalaccel signal alone;
                    // keep it alive.
                    eprintln!("[look] D-Bus service error: {e}");
                    std::future::pending::<()>().await;
                } else {
                    // Every other compositor binding toggles by calling this
                    // service, so the hotkey is dead without it.
                    health::report(
                        health::ISSUE_HOTKEY,
                        format!(
                            "Look's D-Bus service failed to start ({e}), so the \
                             Alt+Space binding cannot reach the app. Restart Look \
                             to retry."
                        ),
                    );
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
    let cmd = toggle_cmd();
    let bound = host_command("swaymsg")
        .arg(format!("bindsym Alt+space exec {cmd}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if bound {
        eprintln!("[look] Registered Sway keybinding: Alt+Space → Look toggle");
    } else {
        health::report(
            health::ISSUE_HOTKEY,
            format!(
                "Failed to register Alt+Space via swaymsg. Bind a key manually \
                 to run: {cmd}"
            ),
        );
    }
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
    let cmd = toggle_cmd();
    let lua = format!(
        r#"pcall(hl.unbind, "ALT + space")
hl.window_rule({{ name = "look-float", match = {{ class = "lookapp" }}, float = true }})
hl.window_rule({{ name = "look-noborder", match = {{ class = "lookapp" }}, border_size = 0, rounding = 0, no_shadow = true }})
hl.bind("ALT + space", hl.dsp.exec_cmd("{cmd}"))"#
    );

    let used_lua = hyprctl_ok(host_command("hyprctl").args(["eval", &lua]).output());

    let bound = used_lua || {
        // Fallback: legacy keyword syntax for older Hyprland
        let _ = host_command("hyprctl")
            .args(["keyword", "windowrulev2", "float, class:lookapp"])
            .output();
        let _ = host_command("hyprctl")
            .args(["keyword", "windowrulev2", "noborder, class:lookapp"])
            .output();
        hyprctl_ok(
            host_command("hyprctl")
                .args(["keyword", "bind", &format!("ALT,space,exec,{cmd}")])
                .output(),
        )
    };

    if bound {
        eprintln!("[look] Registered Hyprland keybinding: Alt+Space → Look toggle");
    } else {
        health::report(
            health::ISSUE_HOTKEY,
            format!(
                "Failed to register Alt+Space via hyprctl. Bind a key manually \
                 to run: {cmd}"
            ),
        );
    }
}

/// hyprctl exits 0 even for some rejected commands, reporting the failure as
/// "error" text on stdout instead - check both.
fn hyprctl_ok(result: std::io::Result<std::process::Output>) -> bool {
    result
        .map(|o| o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("error"))
        .unwrap_or(false)
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
// niri
// ---------------------------------------------------------------------------

/// niri keeps binds in `config.kdl` with no IPC to add one, so the best we can
/// do is hand the user the exact stanza. `spawn` takes argv, not a shell line,
/// so the D-Bus call has to be re-quoted argument by argument.
fn niri_bind_snippet() -> String {
    let argv = toggle_cmd()
        .split_whitespace()
        .map(|arg| format!("\"{arg}\""))
        .collect::<Vec<_>>()
        .join(" ");
    format!("binds {{ Alt+Space {{ spawn {argv}; }} }}")
}

/// Where niri looks for its config, in the order it does.
fn niri_config_paths() -> Vec<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .map(|dir| dir.join("niri/config.kdl"))
        .into_iter()
        .chain([std::path::PathBuf::from("/etc/niri/config.kdl")])
        .collect()
}

/// Whether a bind already spawns something that talks to Look's D-Bus service.
/// Matching on the bus name rather than a full command line keeps this true
/// for any of the three callers, and for a user's own wrapper script.
fn niri_bind_present() -> bool {
    niri_config_paths().iter().any(|path| {
        std::fs::read_to_string(path)
            .map(|config| config.contains(DBUS_NAME))
            .unwrap_or(false)
    })
}

fn report_niri_keybinding() {
    if niri_bind_present() {
        return;
    }
    health::report(
        health::ISSUE_HOTKEY,
        format!(
            "niri has no API to register hotkeys, so Alt+Space must be bound in \
             ~/.config/niri/config.kdl: {}",
            niri_bind_snippet()
        ),
    );
}

// ---------------------------------------------------------------------------
// GNOME
// ---------------------------------------------------------------------------

/// Register a GNOME custom keybinding for Alt+Space → D-Bus call to our service.
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

    let cmd = toggle_cmd();
    let existing = gsettings_get(MEDIA_KEYS_SCHEMA, "custom-keybindings");
    // The stored command counts as part of "bound": an older build wrote a
    // caller this system may no longer have, and only a rewrite fixes it.
    let already_bound = existing.contains(KEYBINDING_PATH)
        && gsettings_get_at(KEYBINDING_SCHEMA, "binding", KEYBINDING_PATH).contains("<Alt>space")
        && gsettings_get_at(KEYBINDING_SCHEMA, "command", KEYBINDING_PATH).contains(cmd);

    // Registered by a previous run and nothing shadowed it: the grab is live.
    if already_bound && !had_conflict {
        return;
    }

    let mut ok = gsettings_set_at(KEYBINDING_SCHEMA, "name", "'Look Toggle'", KEYBINDING_PATH);
    ok &= gsettings_set_at(
        KEYBINDING_SCHEMA,
        "command",
        &format!("'{cmd}'"),
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
    ok &= gsettings_set(MEDIA_KEYS_SCHEMA, "custom-keybindings", &new_value);

    // Write the binding last, toggling it when it was shadowed: dconf drops
    // same-value writes and gsd only re-grabs on a change notification.
    if already_bound {
        gsettings_set_at(KEYBINDING_SCHEMA, "binding", "''", KEYBINDING_PATH);
    }
    ok &= gsettings_set_at(
        KEYBINDING_SCHEMA,
        "binding",
        "'<Alt>space'",
        KEYBINDING_PATH,
    );

    if ok {
        eprintln!("[look] Registered GNOME keybinding: Alt+Space → Look toggle");
    } else {
        health::report(
            health::ISSUE_HOTKEY,
            format!(
                "Failed to register Alt+Space via gsettings. Bind a key manually \
                 to run: {cmd}"
            ),
        );
    }
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
        let cmd = toggle_cmd();
        health::report(
            health::ISSUE_HOTKEY,
            format!(
                "KDE assigned a different key than Alt+Space (it may be taken). \
                 Rebind it in System Settings → Shortcuts → Look, or bind a key \
                 to run: {cmd}"
            ),
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
        // Nothing registered: the niri bind lives in the user's own config.
        Compositor::Niri | Compositor::Other => {}
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

fn gsettings_set(schema: &str, key: &str, value: &str) -> bool {
    host_command("gsettings")
        .args(["set", schema, key, value])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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

fn gsettings_set_at(schema: &str, key: &str, value: &str, path: &str) -> bool {
    host_command("gsettings")
        .args(["set", &format!("{schema}:{path}"), key, value])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn gdbus_command_targets_our_service() {
        assert_eq!(
            toggle_command(CALLER_GDBUS, CALLER_GDBUS),
            "gdbus call --session --dest com.look.Desktop \
             --object-path /com/look/Desktop --method com.look.Desktop.Toggle"
        );
    }

    #[test]
    fn dbus_send_command_targets_our_service() {
        assert_eq!(
            toggle_command(CALLER_DBUS_SEND, CALLER_DBUS_SEND),
            "dbus-send --session --type=method_call --dest=com.look.Desktop \
             /com/look/Desktop com.look.Desktop.Toggle"
        );
    }

    #[test]
    fn busctl_command_targets_our_service() {
        assert_eq!(
            toggle_command(CALLER_BUSCTL, CALLER_BUSCTL),
            "busctl --user call com.look.Desktop /com/look/Desktop com.look.Desktop Toggle"
        );
    }

    #[test]
    fn nix_store_binaries_are_invoked_by_absolute_path() {
        let path = Path::new("/nix/store/abc123-glib-2.84.0/bin/gdbus");
        assert_eq!(
            caller_invocation(CALLER_GDBUS, path),
            "/nix/store/abc123-glib-2.84.0/bin/gdbus"
        );
    }

    #[test]
    fn fhs_binaries_are_invoked_by_bare_name() {
        assert_eq!(
            caller_invocation(CALLER_GDBUS, Path::new("/usr/bin/gdbus")),
            CALLER_GDBUS
        );
    }
}
