//! Focus an existing window on KDE Plasma via KWin's scripting D-Bus API.
//!
//! KWin exposes no direct "activate window by class" D-Bus call, and the
//! wlr-foreign-toplevel protocol used on wlroots compositors is not
//! available on Plasma.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use super::dbus;

const KWIN_BUS: &str = "org.kde.KWin";
const SCRIPTING_PATH: &str = "/Scripting";
const SCRIPTING_IFACE: &str = "org.kde.kwin.Scripting";
const SCRIPT_IFACE: &str = "org.kde.kwin.Script";
const PLUGIN_NAME: &str = "look-focus";
const REPORT_PATH: &str = "/com/look/KWinFocus";
const REPORT_IFACE: &str = "com.look.KWinFocus";
// KWin runs the script on its next event-loop tick; the reply is normally
// near-instant, the timeout only guards against a wedged compositor.
const REPORT_TIMEOUT: Duration = Duration::from_millis(1500);

/// In-flight report channel plus its call token. The KWin script's
/// `callDBus` lands in `FocusReport::report` on zbus's executor thread,
/// which forwards here. The token keeps a late report from a timed-out
/// earlier script from being credited to the current call.
static REPORT_SLOT: Mutex<Option<(String, Sender<bool>)>> = Mutex::new(None);

struct FocusReport;

#[zbus::interface(name = "com.look.KWinFocus")]
impl FocusReport {
    fn report(&self, token: &str, matched: bool) {
        let mut slot = REPORT_SLOT.lock().unwrap();
        if slot.as_ref().is_some_and(|(t, _)| t == token)
            && let Some((_, tx)) = slot.take()
        {
            let _ = tx.send(matched);
        }
    }
}

/// Activate the first KWin window whose resource class or name matches one
/// of `candidates` (case-insensitive).
/// Returns false when nothing matched or
/// the KWin scripting interface is unavailable.
pub fn try_focus(candidates: &[&str]) -> bool {
    let Some(conn) = dbus::session() else {
        return false;
    };
    // One script (and one report slot) in flight at a time.
    static CALL_LOCK: Mutex<()> = Mutex::new(());
    let _guard = CALL_LOCK.lock().unwrap();

    if !ensure_report_object(conn) {
        return false;
    }
    let Some(unique_name) = conn.unique_name().map(|n| n.to_string()) else {
        return false;
    };

    static TOKEN_COUNTER: AtomicU64 = AtomicU64::new(0);
    let token = TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed).to_string();

    let lowered: Vec<String> = candidates.iter().map(|c| c.to_lowercase()).collect();
    let script_path =
        std::env::temp_dir().join(format!("look-kwin-focus-{}.js", std::process::id()));
    if std::fs::write(&script_path, build_script(&unique_name, &token, &lowered)).is_err() {
        return false;
    }

    let (tx, rx) = channel();
    *REPORT_SLOT.lock().unwrap() = Some((token, tx));

    let script_obj = dbus::runtime().block_on(load_and_run(conn, &script_path));
    let matched = script_obj.is_some() && rx.recv_timeout(REPORT_TIMEOUT).unwrap_or(false);

    if let Some(obj) = script_obj {
        dbus::runtime().block_on(async {
            let _ = conn
                .call_method(
                    Some(KWIN_BUS),
                    obj.as_str(),
                    Some(SCRIPT_IFACE),
                    "stop",
                    &(),
                )
                .await;
        });
    }
    *REPORT_SLOT.lock().unwrap() = None;
    let _ = std::fs::remove_file(&script_path);
    eprintln!("[focus] kwin script matched={matched} candidates={lowered:?}");
    matched
}

/// Register the report callback object on the shared connection. Success is
/// cached; a failure is retried on the next call instead of disabling KDE
/// focus for the whole session. Always called with CALL_LOCK held, so the
/// registration cannot race itself.
fn ensure_report_object(conn: &'static zbus::Connection) -> bool {
    static REGISTERED: AtomicBool = AtomicBool::new(false);
    if REGISTERED.load(Ordering::Acquire) {
        return true;
    }
    let ok = dbus::runtime().block_on(async {
        conn.object_server()
            .at(REPORT_PATH, FocusReport)
            .await
            .is_ok()
    });
    if ok {
        REGISTERED.store(true, Ordering::Release);
    }
    ok
}

/// Load the script into KWin and start it. Returns the script's object path
/// on success (needed to stop/unload it afterwards).
async fn load_and_run(conn: &zbus::Connection, path: &std::path::Path) -> Option<String> {
    // A stale plugin with the same name makes loadScript return -1.
    let _ = conn
        .call_method(
            Some(KWIN_BUS),
            SCRIPTING_PATH,
            Some(SCRIPTING_IFACE),
            "unloadScript",
            &(PLUGIN_NAME,),
        )
        .await;

    let reply = conn
        .call_method(
            Some(KWIN_BUS),
            SCRIPTING_PATH,
            Some(SCRIPTING_IFACE),
            "loadScript",
            &(path.to_string_lossy().as_ref(), PLUGIN_NAME),
        )
        .await
        .ok()?;
    let (id,): (i32,) = reply.body().deserialize().ok()?;
    if id < 0 {
        return None;
    }

    // Plasma 6 registers the script object at /Scripting/ScriptN, Plasma 5
    // at /N, so try both.
    for obj in [format!("/Scripting/Script{id}"), format!("/{id}")] {
        if conn
            .call_method(Some(KWIN_BUS), obj.as_str(), Some(SCRIPT_IFACE), "run", &())
            .await
            .is_ok()
        {
            return Some(obj);
        }
    }
    None
}

/// KWin script that activates the first window matching `candidates` and
/// reports the result back over D-Bus. `workspace.windowList` only exists on
/// Plasma 6; its absence selects the Plasma 5 API.
fn build_script(service: &str, token: &str, candidates: &[String]) -> String {
    let list = serde_json::to_string(candidates).unwrap_or_else(|_| "[]".into());
    format!(
        r#"var candidates = {list};
function norm(v) {{ return (v === null || v === undefined) ? "" : v.toString().toLowerCase(); }}
var plasma6 = typeof workspace.windowList === "function";
var wins = plasma6 ? workspace.windowList() : workspace.clientList();
var matched = false;
for (var i = 0; i < wins.length; ++i) {{
    var w = wins[i];
    if (w.skipTaskbar) continue;
    if (candidates.indexOf(norm(w.resourceClass)) < 0 && candidates.indexOf(norm(w.resourceName)) < 0) continue;
    if (plasma6) {{ workspace.activeWindow = w; }} else {{ workspace.activateClient(w); }}
    matched = true;
    break;
}}
callDBus("{service}", "{REPORT_PATH}", "{REPORT_IFACE}", "Report", "{token}", matched);
"#
    )
}
