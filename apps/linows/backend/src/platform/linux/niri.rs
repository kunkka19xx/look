//! Focus and window control on niri via its own IPC socket.
//!
//! niri exposes a line-oriented JSON protocol on `$NIRI_SOCKET`: write one
//! serialised request, read one reply. Talking to the socket directly avoids
//! spawning `niri msg` per call, and unlike the generic
//! `wlr-foreign-toplevel` `activate` request, `FocusWindow` also scrolls the
//! view to the window's workspace - which is the whole point on a
//! scrolling-tiling compositor where the match may sit on another workspace.

use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Cap on a single request/reply exchange. The socket is local and niri
/// answers immediately; a timeout only matters when the compositor is wedged,
/// where blocking the caller would freeze the launcher.
const IPC_TIMEOUT: Duration = Duration::from_millis(300);

/// `app_id` niri sees for Look's own window (matches the Tauri identifier).
const SELF_APP_ID: &str = "lookapp";

/// How long to wait for niri to map Look's toplevel after `show()`.
const MAP_WAIT: Duration = Duration::from_millis(600);
const MAP_POLL_INTERVAL: Duration = Duration::from_millis(30);

#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "Ok")]
    ok: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct Window {
    pub id: u64,
    pub app_id: Option<String>,
    pub is_floating: bool,
    pub focus_timestamp: Option<Timestamp>,
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    secs: u64,
    nanos: u32,
}

fn request(payload: &serde_json::Value) -> Option<serde_json::Value> {
    let path = std::env::var_os("NIRI_SOCKET")?;
    let stream = UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(IPC_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(IPC_TIMEOUT)).ok()?;

    let mut writer = &stream;
    writeln!(writer, "{payload}").ok()?;
    writer.flush().ok()?;

    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).ok()?;
    serde_json::from_str::<Reply>(&line).ok()?.ok
}

/// Every window niri currently manages.
pub fn windows() -> Vec<Window> {
    request(&serde_json::json!("Windows"))
        .and_then(|v| serde_json::from_value(v.get("Windows")?.clone()).ok())
        .unwrap_or_default()
}

fn action(name: &str, id: u64) -> bool {
    request(&serde_json::json!({ "Action": { name: { "id": id } } }))
        .is_some_and(|v| v == serde_json::json!("Handled"))
}

/// Pick the window to focus: candidates are tried in caller order (most
/// specific first), and among several windows of the same app the most
/// recently focused one wins, so repeated invocations land where the user
/// left off rather than on whichever window niri happens to list first.
fn pick<'a>(open: &'a [Window], candidates: &[&str]) -> Option<&'a Window> {
    candidates.iter().find_map(|candidate| {
        open.iter()
            .filter(|w| {
                w.app_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(candidate))
            })
            .max_by(|a, b| a.focus_timestamp.cmp(&b.focus_timestamp))
    })
}

/// Focus the open window matching one of the candidate `app_id`s, scrolling
/// to its workspace.
pub fn try_focus(candidates: &[&str]) -> bool {
    let open = windows();
    match pick(&open, candidates) {
        Some(window) => {
            eprintln!(
                "[focus] niri FocusWindow id={} app_id={:?}",
                window.id, window.app_id
            );
            action("FocusWindow", window.id)
        }
        None => {
            eprintln!("[focus] niri: no window matched {candidates:?}");
            false
        }
    }
}

/// Move Look's own window into the floating layout, off the show path.
///
/// Without this niri tiles the launcher into the scrolling row, resizing
/// whatever the user was working in every time it opens. Hiding destroys the
/// surface, so niri gives the window a fresh id on every show and the rule has
/// to be re-applied; the poll covers the gap between `show()` returning and
/// the compositor mapping the new toplevel. A user `window-rule` that already
/// floats Look wins the race and this call then does nothing.
pub fn ensure_self_floating() {
    std::thread::spawn(|| {
        let deadline = std::time::Instant::now() + MAP_WAIT;
        while std::time::Instant::now() < deadline {
            if let Some(window) = windows()
                .into_iter()
                .find(|w| w.app_id.as_deref() == Some(SELF_APP_ID))
            {
                if !window.is_floating {
                    action("MoveWindowToFloating", window.id);
                }
                return;
            }
            std::thread::sleep(MAP_POLL_INTERVAL);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u64, app_id: &str, focused_at: Option<u64>) -> Window {
        Window {
            id,
            app_id: Some(app_id.into()),
            is_floating: false,
            focus_timestamp: focused_at.map(|secs| Timestamp { secs, nanos: 0 }),
        }
    }

    #[test]
    fn matches_app_id_case_insensitively() {
        let open = [window(1, "brave-browser", Some(1))];
        assert_eq!(pick(&open, &["Brave-Browser"]).map(|w| w.id), Some(1));
    }

    #[test]
    fn earlier_candidates_win() {
        let open = [window(1, "firefox", Some(1)), window(2, "zen", Some(9))];
        assert_eq!(pick(&open, &["firefox", "zen"]).map(|w| w.id), Some(1));
    }

    #[test]
    fn picks_most_recently_focused_of_the_same_app() {
        let open = [
            window(1, "brave-browser", Some(5)),
            window(2, "brave-browser", Some(9)),
            window(3, "brave-browser", None),
        ];
        assert_eq!(pick(&open, &["brave-browser"]).map(|w| w.id), Some(2));
    }

    #[test]
    fn no_match_focuses_nothing() {
        let open = [window(1, "firefox", Some(1))];
        assert!(pick(&open, &["brave-browser"]).is_none());
        assert!(pick(&[], &["firefox"]).is_none());
    }
}
