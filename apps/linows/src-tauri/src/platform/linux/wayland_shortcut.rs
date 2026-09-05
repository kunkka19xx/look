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

/// Runs the toggle on a thread of its own rather than on whichever one
/// signalled it.
///
/// Both signals - our `Toggle` method and KDE's kglobalaccel - are delivered
/// inside the tokio runtime `start` builds below, and showing the window makes
/// D-Bus calls of its own, which cannot block on a runtime from within one.
/// Answering off the runtime also returns the D-Bus call immediately instead of
/// holding the caller for as long as the window takes to place and map.
///
/// One worker rather than one thread per signal keeps toggles in the order they
/// arrived: a fast hide/show pair must not land reversed.
fn off_runtime<F>(on_toggle: F) -> impl Fn() + Send + Sync + 'static
where
    F: Fn() + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            on_toggle();
        }
    });
    // A Sender is Send but not Sync, and the D-Bus service holds its callback
    // behind a shared reference.
    let tx = Mutex::new(tx);
    move || {
        if let Ok(tx) = tx.lock() {
            let _ = tx.send(());
        }
    }
}

/// Start a background thread that:
/// 1. Registers a compositor-specific keybinding for Alt+Space
/// 2. Registers a D-Bus service to listen for Toggle calls
pub fn start<F>(on_toggle: F)
where
    F: Fn() + Send + Sync + 'static,
{
    let compositor = detect_compositor();
    let on_toggle = off_runtime(on_toggle);

    std::thread::spawn(move || {
        // Reported before registration: health::report keeps the first message
        // per issue id, and a missing caller explains the dead key better than
        // the per-compositor failures that follow.
        if compositor != Compositor::Kde && dbus_caller().is_none() {
            health::report_as(
                health::ISSUE_HOTKEY,
                "no-dbus-tool",
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
                health::report_as(
                    health::ISSUE_HOTKEY,
                    "no-api",
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
                        health::report_as(
                            health::ISSUE_HOTKEY,
                            "kde-failed",
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
                    health::report_as(
                        health::ISSUE_HOTKEY,
                        "dbus-service",
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

const SWAY_KEY: &str = "Alt+space";

/// Sway drops its own bindings while the focused window holds a
/// keyboard-shortcuts inhibitor, which fullscreen browsers, games and VMs all
/// take; `--inhibited` exempts ours. Bind and unbind must agree on it, since
/// sway matches bindings on flags as well as keys.
const SWAY_BIND_FLAGS: &str = "--inhibited";

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

    // Drop a binding left by a Look old enough to predate SWAY_BIND_FLAGS:
    // sway counts the flag as part of a binding's identity, so the two would
    // coexist and fire the toggle twice.
    let _ = host_command("swaymsg")
        .arg(format!("unbindsym {SWAY_KEY}"))
        .output();

    // Bind Alt+Space to toggle Look via D-Bus
    let cmd = toggle_cmd();
    let bound = host_command("swaymsg")
        .arg(format!("bindsym {SWAY_BIND_FLAGS} {SWAY_KEY} exec {cmd}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if bound {
        eprintln!("[look] Registered Sway keybinding: Alt+Space → Look toggle");
    } else {
        health::report_as(
            health::ISSUE_HOTKEY,
            "sway-failed",
            format!(
                "Failed to register Alt+Space via swaymsg. Bind a key manually \
                 to run: {cmd}"
            ),
        );
    }
}

fn cleanup_sway_keybinding() {
    let _ = host_command("swaymsg")
        .arg(format!("unbindsym {SWAY_BIND_FLAGS} {SWAY_KEY}"))
        .output();
    eprintln!("[look] Removed Sway keybinding for Alt+Space");
}

// ---------------------------------------------------------------------------
// Hyprland
// ---------------------------------------------------------------------------

fn ensure_hyprland_keybinding() {
    let cmd = toggle_cmd();

    // `dont_inhibit` keeps Alt+Space alive while the focused window holds a
    // keyboard-shortcuts inhibitor, as fullscreen browsers, games and VMs do.
    // Hyprland builds predating the flag reject the whole bind, so a plain one
    // follows.
    let bound = hyprland_bind(cmd, true) || hyprland_bind(cmd, false);

    if bound {
        eprintln!("[look] Registered Hyprland keybinding: Alt+Space → Look toggle");
    } else {
        health::report_as(
            health::ISSUE_HOTKEY,
            "hypr-failed",
            format!(
                "Failed to register Alt+Space via hyprctl. Bind a key manually \
                 to run: {cmd}"
            ),
        );
    }
}

/// One bind attempt over both config dialects: Hyprland v0.55+ takes Lua via
/// `hyprctl eval` with the hl.* API, older versions the INI-style
/// `hyprctl keyword`, where `bindp` spells `dont_inhibit`.
///
/// hl.bind stacks duplicates on every call (hot-reloads in dev, or sequential
/// launches in prod), so unbind first via pcall - pcall keeps the eval
/// succeeding even when the binding doesn't exist yet (first run).
fn hyprland_bind(cmd: &str, dont_inhibit: bool) -> bool {
    let flags = if dont_inhibit {
        ", { dont_inhibit = true }"
    } else {
        ""
    };
    let lua = format!(
        r#"pcall(hl.unbind, "ALT + space")
hl.window_rule({{ name = "look-float", match = {{ class = "lookapp" }}, float = true }})
hl.window_rule({{ name = "look-noborder", match = {{ class = "lookapp" }}, border_size = 0, rounding = 0, no_shadow = true }})
hl.bind("ALT + space", hl.dsp.exec_cmd("{cmd}"){flags})"#
    );
    if hyprctl_ok(host_command("hyprctl").args(["eval", &lua]).output()) {
        return true;
    }

    // The Lua path opens with an unbind; this one has to as well. `keyword
    // bind` appends rather than replaces, so an ALT,space bind left by an
    // earlier run - or by the other of bind/bindp - stays live alongside the
    // new one and Hyprland fires both dispatchers.
    let _ = host_command("hyprctl")
        .args(["keyword", "unbind", "ALT,space"])
        .output();
    let _ = host_command("hyprctl")
        .args(["keyword", "windowrulev2", "float, class:lookapp"])
        .output();
    let _ = host_command("hyprctl")
        .args(["keyword", "windowrulev2", "noborder, class:lookapp"])
        .output();
    hyprctl_ok(
        host_command("hyprctl")
            .args([
                "keyword",
                if dont_inhibit { "bindp" } else { "bind" },
                &format!("ALT,space,exec,{cmd}"),
            ])
            .output(),
    )
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
    format!("binds {{ Alt+Space {NIRI_NO_INHIBIT} {{ spawn {argv}; }} }}")
}

/// Read only when nothing above it names a config.
const NIRI_SYSTEM_CONFIG: &str = "/etc/niri/config.kdl";

/// The one config niri actually loaded. It reads a single root and never
/// merges, so every other candidate has to stay unread: a bind sitting in a
/// file niri ignores would otherwise pass for a working hotkey and suppress
/// the setup notice the user needs.
fn niri_config_root() -> std::path::PathBuf {
    let pid = niri_pid();
    select_niri_root(
        pid.and_then(niri_config_flag),
        std::env::var_os("NIRI_CONFIG").map(std::path::PathBuf::from),
        niri_user_config().filter(|path| path.is_file()),
        pid.and_then(niri_cwd),
    )
}

/// niri's order: `--config` beats `NIRI_CONFIG` ("if both are set, the command
/// line argument takes precedence"), either beats the user's file, and the
/// system file is the last resort. `user` is `None` unless it exists, since
/// niri falls through to the system file when it does not. `cwd` is niri's
/// working directory, which only the two explicit paths are resolved against.
fn select_niri_root(
    flag: Option<std::path::PathBuf>,
    env: Option<std::path::PathBuf>,
    user: Option<std::path::PathBuf>,
    cwd: Option<std::path::PathBuf>,
) -> std::path::PathBuf {
    [flag, env]
        .into_iter()
        .flatten()
        // An exported-but-empty NIRI_CONFIG names nothing; niri treats it as
        // unset and so must we, or we scan "" and report a missing bind.
        .find(|path| !path.as_os_str().is_empty())
        // A relative `--config`/`NIRI_CONFIG` is niri's own working directory,
        // not ours; joining ours would read a different file or none at all.
        .map(|path| match cwd {
            Some(cwd) if path.is_relative() => cwd.join(path),
            _ => path,
        })
        .or(user)
        .unwrap_or_else(|| std::path::PathBuf::from(NIRI_SYSTEM_CONFIG))
}

fn niri_user_config() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .map(|dir| dir.join("niri/config.kdl"))
}

/// The compositor's pid, off the socket path - `niri.<display>.<pid>.sock` -
/// since nothing else exports it. Silently `None` anywhere that shape does not
/// hold, which leaves every caller on its own fallback.
fn niri_pid() -> Option<u32> {
    let socket = std::env::var("NIRI_SOCKET").ok()?;
    std::path::Path::new(&socket)
        .file_stem()?
        .to_str()?
        .rsplit('.')
        .next()?
        .parse()
        .ok()
}

/// `-c`/`--config` off niri's own command line, which nothing exports.
fn niri_config_flag(pid: u32) -> Option<std::path::PathBuf> {
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    niri_config_arg(
        cmdline
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .map(String::from_utf8_lossy),
    )
}

/// Where niri resolves its relative config path from. Look is not started from
/// the same directory - systemd units and `spawn-at-startup` both land in the
/// home - so this cannot be assumed to be ours.
fn niri_cwd(pid: u32) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

/// Last one wins, as clap resolves a repeated single-value argument.
fn niri_config_arg<'a>(
    args: impl Iterator<Item = std::borrow::Cow<'a, str>>,
) -> Option<std::path::PathBuf> {
    let mut args = args.skip(1);
    let mut found = None;
    while let Some(arg) = args.next() {
        if let Some(path) = arg.strip_prefix("--config=") {
            found = Some(std::path::PathBuf::from(path));
        } else if arg == "-c" || arg == "--config" {
            found = args
                .next()
                .map(|path| std::path::PathBuf::from(path.as_ref()));
        }
    }
    found
}

/// Ceiling on `include` nesting, matching niri's own recursion limit.
const NIRI_INCLUDE_DEPTH: u8 = 10;

/// Bind property that keeps niri handling the key itself instead of passing it
/// to a window holding a keyboard-shortcuts inhibitor.
const NIRI_NO_INHIBIT: &str = "allow-inhibiting=false";

/// What the user's config does about Look's hotkey.
#[derive(Debug, PartialEq)]
enum NiriBind {
    /// No bind spawns our D-Bus call.
    Missing,
    /// Bound, but fullscreen windows can swallow the key.
    Inhibitable,
    /// Bound and exempt from inhibiting.
    Ready,
}

/// Files an `include` node pulls in, resolved as niri resolves them: one
/// quoted path per node, relative to the including file, `~` for `$HOME`,
/// no globs. The path is the only quoted token on the line, so an
/// `optional=true` property in front of it needs no parsing of its own.
fn niri_includes(config: &str, dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    config
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("include")?;
            let (_, quoted) = rest.strip_prefix(char::is_whitespace)?.split_once('"')?;
            resolve_niri_include(quoted.split_once('"')?.0, dir)
        })
        .collect()
}

fn resolve_niri_include(arg: &str, dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let path = match arg.strip_prefix("~/") {
        Some(rest) => std::path::PathBuf::from(std::env::var_os("HOME")?).join(rest),
        None => std::path::PathBuf::from(arg),
    };
    Some(if path.is_absolute() {
        path
    } else {
        dir.join(path)
    })
}

/// What, if anything, the config binds to Look's D-Bus service. Matching on the
/// bus name rather than a full command line finds the bind for any of the three
/// callers, and for a user's own wrapper script. Included files count:
/// splitting binds into their own `.kdl` is common, and missing one means
/// nagging a user whose hotkey already works.
fn niri_bind_state_in(root: std::path::PathBuf) -> NiriBind {
    // A queue, not a stack: niri applies includes in the order it meets them,
    // and the header we read the opt-out off has to be the one it would use.
    let mut pending = std::collections::VecDeque::from([(root, 0u8)]);
    let mut seen = std::collections::HashSet::new();

    while let Some((path, depth)) = pending.pop_front() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let Ok(config) = std::fs::read_to_string(&path) else {
            continue;
        };
        if config.contains(DBUS_NAME) {
            return niri_bind_inhibitable(&config);
        }
        if depth < NIRI_INCLUDE_DEPTH {
            let dir = path.parent().unwrap_or(std::path::Path::new(""));
            pending.extend(
                niri_includes(&config, dir)
                    .into_iter()
                    .map(|path| (path, depth + 1)),
            );
        }
    }
    NiriBind::Missing
}

/// Whether the bind spawning our D-Bus call opts out of shortcut inhibiting.
/// KDL keeps a node's properties on its opening line, so the header is the
/// last `{` line at or above the one naming the bus - a `spawn` argv split
/// over several lines sits below it. A header we cannot identify reports
/// `Ready` rather than nagging about a hotkey that may well be fine.
fn niri_bind_inhibitable(config: &str) -> NiriBind {
    let mut header = None;
    for line in config.lines() {
        if line.contains('{') {
            header = Some(line);
        }
        if line.contains(DBUS_NAME) {
            break;
        }
    }
    match header {
        Some(line) if !line.contains(NIRI_NO_INHIBIT) => NiriBind::Inhibitable,
        _ => NiriBind::Ready,
    }
}

fn report_niri_keybinding() {
    // Named in both notices: with `--config`, `NIRI_CONFIG` or no user file at
    // all, the path we read is not the one the user would guess.
    let root = niri_config_root();
    let state = niri_bind_state_in(root.clone());
    let root = root.display();
    match state {
        NiriBind::Ready => {}
        NiriBind::Inhibitable => health::report_as(
            health::ISSUE_HOTKEY,
            "niri-inhibitable",
            format!(
                "niri hands Alt+Space to fullscreen windows that inhibit \
                 keyboard shortcuts (games, browsers, virtual machines), so \
                 Look will not open over them. Add {NIRI_NO_INHIBIT} to the \
                 bind in {root}: {}",
                niri_bind_snippet()
            ),
        ),
        NiriBind::Missing => health::report_as(
            health::ISSUE_HOTKEY,
            "niri-missing",
            format!(
                "niri has no API to register hotkeys, so Alt+Space must be bound in \
                 {root}, or any file it includes: {}",
                niri_bind_snippet()
            ),
        ),
    }
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
        health::report_as(
            health::ISSUE_HOTKEY,
            "gsettings-failed",
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
        health::report_as(
            health::ISSUE_HOTKEY,
            "kde-rebound",
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
    use std::borrow::Cow;
    use std::path::{Path, PathBuf};

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

    #[test]
    fn includes_resolve_against_the_including_file() {
        assert_eq!(
            niri_includes("include \"binds.kdl\"", Path::new("/home/u/.config/niri")),
            [Path::new("/home/u/.config/niri/binds.kdl")]
        );
    }

    #[test]
    fn absolute_includes_are_taken_as_is() {
        assert_eq!(
            niri_includes(
                "  include   \"/etc/niri/binds.kdl\" // shared",
                Path::new("/tmp")
            ),
            [Path::new("/etc/niri/binds.kdl")]
        );
    }

    #[test]
    fn tilde_includes_expand_to_home() {
        let home = std::env::var("HOME").expect("HOME");
        assert_eq!(
            niri_includes("include \"~/dots/binds.kdl\"", Path::new("/tmp")),
            [Path::new(&home).join("dots/binds.kdl")]
        );
    }

    #[test]
    fn a_bind_in_an_included_file_counts_as_present() {
        let dir = std::env::temp_dir().join("look-niri-include-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).expect("temp dir");
        std::fs::write(dir.join("config.kdl"), "include \"nested/binds.kdl\"\n").expect("config");
        std::fs::write(
            dir.join("nested/binds.kdl"),
            format!(
                "binds {{ Alt+Space {NIRI_NO_INHIBIT} {{ spawn \"gdbus\" \"{DBUS_NAME}\"; }} }}"
            ),
        )
        .expect("binds");

        assert_eq!(niri_bind_state_in(dir.join("config.kdl")), NiriBind::Ready);

        std::fs::write(dir.join("nested/binds.kdl"), "binds { }").expect("binds");
        assert_eq!(
            niri_bind_state_in(dir.join("config.kdl")),
            NiriBind::Missing
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An exported-but-empty NIRI_CONFIG names nothing. Reading it as a path
    /// finds no bind and nags a user whose hotkey already works.
    #[test]
    fn an_empty_niri_config_env_falls_through_to_the_user_file() {
        let user = PathBuf::from("/home/u/.config/niri/config.kdl");
        assert_eq!(
            select_niri_root(None, Some(PathBuf::new()), Some(user.clone()), None),
            user
        );
    }

    #[test]
    fn the_user_file_wins_over_the_system_one_and_only_when_it_exists() {
        let user = PathBuf::from("/home/u/.config/niri/config.kdl");
        assert_eq!(select_niri_root(None, None, Some(user.clone()), None), user);
        assert_eq!(
            select_niri_root(None, None, None, None),
            PathBuf::from(NIRI_SYSTEM_CONFIG)
        );
    }

    /// niri: "if both are set, the command line argument takes precedence".
    #[test]
    fn the_command_line_config_outranks_the_env_and_the_user_file() {
        assert_eq!(
            select_niri_root(
                Some(PathBuf::from("/flag.kdl")),
                Some(PathBuf::from("/env.kdl")),
                Some(PathBuf::from("/user.kdl")),
                None,
            ),
            PathBuf::from("/flag.kdl")
        );
        assert_eq!(
            select_niri_root(
                None,
                Some(PathBuf::from("/env.kdl")),
                Some(PathBuf::from("/user.kdl")),
                None,
            ),
            PathBuf::from("/env.kdl")
        );
    }

    /// niri reads a relative `--config`/`NIRI_CONFIG` from its own working
    /// directory. Look runs from a different one, so keeping the path as given
    /// scans the wrong file and nags about a hotkey that already works.
    #[test]
    fn a_relative_explicit_config_resolves_against_niris_working_directory() {
        let cwd = PathBuf::from("/run/niri");
        assert_eq!(
            select_niri_root(
                Some(PathBuf::from("niri.kdl")),
                None,
                None,
                Some(cwd.clone())
            ),
            cwd.join("niri.kdl")
        );
        assert_eq!(
            select_niri_root(
                None,
                Some(PathBuf::from("cfg/niri.kdl")),
                None,
                Some(cwd.clone())
            ),
            cwd.join("cfg/niri.kdl")
        );
        // Absolute paths and the user/system fallbacks are untouched by it.
        assert_eq!(
            select_niri_root(
                Some(PathBuf::from("/flag.kdl")),
                None,
                None,
                Some(cwd.clone())
            ),
            PathBuf::from("/flag.kdl")
        );
        let user = PathBuf::from("/home/u/.config/niri/config.kdl");
        assert_eq!(
            select_niri_root(None, None, Some(user.clone()), Some(cwd)),
            user
        );
    }

    #[test]
    fn the_config_flag_is_read_off_niri_argv_in_either_form() {
        let arg = |args: &[&str]| niri_config_arg(args.iter().map(|a| Cow::from(*a)));
        assert_eq!(arg(&["niri", "--session"]), None);
        assert_eq!(
            arg(&["niri", "-c", "/a.kdl"]),
            Some(PathBuf::from("/a.kdl"))
        );
        assert_eq!(
            arg(&["niri", "--config", "/a.kdl"]),
            Some(PathBuf::from("/a.kdl"))
        );
        assert_eq!(
            arg(&["niri", "--config=/a.kdl"]),
            Some(PathBuf::from("/a.kdl"))
        );
        // clap resolves a repeated single-value argument to the last one.
        assert_eq!(
            arg(&["niri", "-c", "/a.kdl", "--config=/b.kdl"]),
            Some(PathBuf::from("/b.kdl"))
        );
    }

    /// The regression the selection exists for: a bind in a config niri never
    /// loads must not pass for a working hotkey.
    #[test]
    fn a_bind_only_in_the_unselected_config_does_not_count() {
        let dir = std::env::temp_dir().join("look-niri-precedence-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let user = dir.join("user.kdl");
        let system = dir.join("system.kdl");
        std::fs::write(&user, "binds { }").expect("user");
        std::fs::write(
            &system,
            format!("binds {{ Alt+Space {NIRI_NO_INHIBIT} {{ spawn \"{DBUS_NAME}\"; }} }}"),
        )
        .expect("system");

        assert_eq!(
            select_niri_root(None, None, Some(user.clone()), None),
            user,
            "the existing user file is the root"
        );
        assert_eq!(niri_bind_state_in(user), NiriBind::Missing);
        assert_eq!(niri_bind_state_in(system), NiriBind::Ready);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bind_without_the_opt_out_is_reported_inhibitable() {
        assert_eq!(
            niri_bind_inhibitable(&multiline_bind("")),
            NiriBind::Inhibitable
        );
    }

    #[test]
    fn the_opt_out_on_the_bind_header_counts_for_a_multiline_spawn() {
        assert_eq!(
            niri_bind_inhibitable(&multiline_bind(NIRI_NO_INHIBIT)),
            NiriBind::Ready
        );
    }

    /// A bind whose `spawn` argv wraps, so the bus name lands well below the
    /// header carrying the properties.
    fn multiline_bind(props: &str) -> String {
        [
            "binds {".to_string(),
            format!("    Alt+Space {props} {{"),
            "        spawn \"dbus-send\" \\".to_string(),
            format!("              \"--dest={DBUS_NAME}\" \\"),
            format!("              \"{DBUS_NAME}.{TOGGLE_METHOD}\""),
            "    }".to_string(),
            "}".to_string(),
        ]
        .join("\n")
    }

    #[test]
    fn cyclic_includes_terminate() {
        let dir = std::env::temp_dir().join("look-niri-cycle-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("a.kdl"), "include \"b.kdl\"\n").expect("a");
        std::fs::write(dir.join("b.kdl"), "include \"a.kdl\"\n").expect("b");

        assert_eq!(niri_bind_state_in(dir.join("a.kdl")), NiriBind::Missing);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn optional_includes_are_followed_too() {
        assert_eq!(
            niri_includes(
                "include optional=true \"local.kdl\"",
                Path::new("/etc/niri")
            ),
            [Path::new("/etc/niri/local.kdl")]
        );
    }

    #[test]
    fn non_include_lines_pull_in_nothing() {
        let config =
            "// include \"old.kdl\"\nincludes \"x.kdl\"\nbinds { Alt+Space { spawn \"x\"; } }";
        assert!(niri_includes(config, Path::new("/tmp")).is_empty());
    }
}
