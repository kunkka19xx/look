//! Now Playing for the launchpad transport tile. Mirrors the macOS
//! `SystemNowPlaying`, but reads the standard Linux media protocol (MPRIS over
//! the session bus) instead of a private framework: every well-behaved player
//! (browsers, Spotify, mpv, ...) exports `org.mpris.MediaPlayer2.*`, so a glance
//! and the prev/play-next transport work without per-player integration.
//!
//! The app's own background music (rodio) does not export MPRIS, so it does not
//! appear here; this tile reflects whatever external player owns the media keys.

use serde::Serialize;

/// A snapshot of the current MPRIS track, whatever app owns it.
#[derive(Debug, Clone, Serialize)]
pub struct NowPlayingSnapshot {
    pub title: String,
    pub artist: Option<String>,
    /// The owning app's display name (MPRIS `Identity`), e.g. "Firefox".
    pub app: Option<String>,
    pub is_playing: bool,
    /// Handle passed back with a transport command so control drives this exact
    /// player (MPRIS bus name). `None` on Windows (one implicit SMTC session).
    pub player: Option<String>,
}

/// The current track, or `None` when nothing is playing. Runs the blocking
/// D-Bus read off the UI thread.
#[tauri::command]
pub async fn now_playing_current() -> Option<NowPlayingSnapshot> {
    tauri::async_runtime::spawn_blocking(current)
        .await
        .ok()
        .flatten()
}

/// Send a transport command (`"playpause"`, `"next"`, `"previous"`) to a player.
/// `player` is the handle from the snapshot being controlled; when omitted the
/// active player is re-picked. Returns whether it was delivered.
#[tauri::command]
pub async fn now_playing_command(command: String, player: Option<String>) -> bool {
    tauri::async_runtime::spawn_blocking(move || run_command(&command, player.as_deref()))
        .await
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
mod imp {
    use super::NowPlayingSnapshot;
    use crate::platform::linux::dbus;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use zbus::zvariant::{OwnedValue, Str};

    const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
    const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
    const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
    const ROOT_IFACE: &str = "org.mpris.MediaPlayer2";
    const PLAYING: &str = "Playing";

    /// The player the tile last locked onto, so a background player that starts
    /// later can't steal it while the current one is still playing.
    static LAST_ACTIVE: Mutex<Option<String>> = Mutex::new(None);

    pub fn current() -> Option<NowPlayingSnapshot> {
        let conn = dbus::session()?;
        dbus::runtime().block_on(async {
            let (name, status) = active_player(conn).await?;
            read_snapshot(conn, &name, &status).await
        })
    }

    pub fn run_command(command: &str, target: Option<&str>) -> bool {
        let method = match command {
            "playpause" => "PlayPause",
            "next" => "Next",
            "previous" => "Previous",
            _ => return false,
        };
        let Some(conn) = dbus::session() else {
            return false;
        };
        dbus::runtime().block_on(async {
            let name = match target {
                Some(name) => name.to_string(),
                None => match active_player(conn).await {
                    Some((name, _)) => name,
                    None => return false,
                },
            };
            let Some(proxy) = player_proxy(conn, &name, PLAYER_IFACE).await else {
                return false;
            };
            proxy.call_method(method, &()).await.is_ok()
        })
    }

    /// An uncached proxy for a one-shot read/call. The default proxy sets up a
    /// `PropertiesChanged` subscription on first property access, pure churn for
    /// the short-lived proxies this module builds every 2s poll.
    async fn player_proxy<'a>(
        conn: &'a zbus::Connection,
        name: &str,
        interface: &'static str,
    ) -> Option<zbus::Proxy<'a>> {
        zbus::proxy::Builder::new(conn)
            .destination(name.to_string())
            .ok()?
            .path(MPRIS_PATH)
            .ok()?
            .interface(interface)
            .ok()?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .ok()
    }

    /// The player to display and drive, with its status. Skips uncontrollable
    /// instances; remembers the pick so the next poll can favour it.
    async fn active_player(conn: &zbus::Connection) -> Option<(String, String)> {
        let reply = conn
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "ListNames",
                &(),
            )
            .await
            .ok()?;
        let names: Vec<String> = reply.body().deserialize().ok()?;

        let mut candidates: Vec<(String, String)> = Vec::new();
        for name in names.into_iter().filter(|n| n.starts_with(MPRIS_PREFIX)) {
            if let Some((status, true)) = player_state(conn, &name).await {
                candidates.push((name, status));
            }
        }
        if candidates.is_empty() {
            return None;
        }

        let mut last = LAST_ACTIVE.lock().ok()?;
        let name = pick_player(&candidates, last.as_deref());
        let status = candidates
            .iter()
            .find(|(c, _)| *c == name)
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        *last = Some(name.clone());
        Some((name, status))
    }

    /// Locked player if still playing, else any playing, else locked if still
    /// present (keeps pause/resume put), else the first. `candidates` non-empty.
    fn pick_player(candidates: &[(String, String)], locked: Option<&str>) -> String {
        let playing = |name: &str| candidates.iter().any(|(c, s)| c == name && s == PLAYING);
        if let Some(name) = locked.filter(|n| playing(n)) {
            return name.to_string();
        }
        if let Some((name, _)) = candidates.iter().find(|(_, s)| s == PLAYING) {
            return name.clone();
        }
        if let Some(name) = locked.filter(|n| candidates.iter().any(|(c, _)| c == n)) {
            return name.to_string();
        }
        candidates[0].0.clone()
    }

    /// A player's `(PlaybackStatus, CanControl)` in one proxy. `CanControl`
    /// defaults to `true` so a player that omits it isn't hidden.
    async fn player_state(conn: &zbus::Connection, name: &str) -> Option<(String, bool)> {
        let proxy = player_proxy(conn, name, PLAYER_IFACE).await?;
        let status = proxy.get_property::<String>("PlaybackStatus").await.ok()?;
        let can_control = proxy
            .get_property::<bool>("CanControl")
            .await
            .unwrap_or(true);
        Some((status, can_control))
    }

    async fn read_snapshot(
        conn: &zbus::Connection,
        name: &str,
        status: &str,
    ) -> Option<NowPlayingSnapshot> {
        let metadata: HashMap<String, OwnedValue> = player_proxy(conn, name, PLAYER_IFACE)
            .await?
            .get_property("Metadata")
            .await
            .ok()?;

        let title = metadata.get("xesam:title").and_then(one_string)?;
        if title.is_empty() {
            return None;
        }
        let artist = metadata
            .get("xesam:artist")
            .and_then(joined_strings)
            .filter(|s| !s.is_empty());
        // A missing app name just leaves `app` None; it must not blank the tile.
        let app = match player_proxy(conn, name, ROOT_IFACE).await {
            Some(proxy) => proxy.get_property::<String>("Identity").await.ok(),
            None => None,
        };

        Some(NowPlayingSnapshot {
            title,
            artist,
            app,
            is_playing: status == PLAYING,
            player: Some(name.to_string()),
        })
    }

    /// A single string metadata value (e.g. `xesam:title`).
    fn one_string(value: &OwnedValue) -> Option<String> {
        value.downcast_ref::<Str>().ok().map(|s| s.to_string())
    }

    /// A string-or-array metadata value (e.g. `xesam:artist`), joined for
    /// display. Handles both the common array form and a plain string.
    fn joined_strings(value: &OwnedValue) -> Option<String> {
        if let Ok(single) = value.downcast_ref::<Str>() {
            return Some(single.to_string());
        }
        let array = value.downcast_ref::<zbus::zvariant::Array>().ok()?;
        let parts: Vec<String> = array
            .iter()
            .filter_map(|item| item.downcast_ref::<Str>().ok().map(|s| s.to_string()))
            .collect();
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}

/// Now Playing via the System Media Transport Controls (SMTC), the Windows peer
/// of MPRIS. `GlobalSystemMediaTransportControlsSessionManager` surfaces whatever
/// app currently owns the media keys (browsers, Spotify, Groove, ...), so a
/// glance and the transport work without per-app integration.
#[cfg(target_os = "windows")]
mod imp {
    use super::NowPlayingSnapshot;
    use crate::platform::windows::ensure_mta;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as SessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    };

    /// The session currently owning the media keys, or `None` when nothing is.
    fn active_session() -> Option<Session> {
        ensure_mta();
        let manager = SessionManager::RequestAsync()
            .and_then(|op| op.get())
            .ok()?;
        manager.GetCurrentSession().ok()
    }

    pub fn current() -> Option<NowPlayingSnapshot> {
        let session = active_session()?;

        let is_playing = session
            .GetPlaybackInfo()
            .and_then(|info| info.PlaybackStatus())
            .map(|status| status == PlaybackStatus::Playing)
            .unwrap_or(false);

        let props = session
            .TryGetMediaPropertiesAsync()
            .and_then(|op| op.get())
            .ok()?;

        let title = props.Title().ok()?.to_string();
        if title.is_empty() {
            return None;
        }
        let artist = props
            .Artist()
            .ok()
            .map(|a| a.to_string())
            .filter(|a| !a.is_empty());
        // A missing app name just leaves `app` None; it must not blank the tile.
        let app = session
            .SourceAppUserModelId()
            .ok()
            .map(|id| app_name(&id.to_string()))
            .filter(|a| !a.is_empty());

        Some(NowPlayingSnapshot {
            title,
            artist,
            app,
            is_playing,
            player: None, // SMTC drives one implicit session.
        })
    }

    pub fn run_command(command: &str, _target: Option<&str>) -> bool {
        let Some(session) = active_session() else {
            return false;
        };
        let op = match command {
            "playpause" => session.TryTogglePlayPauseAsync(),
            "next" => session.TrySkipNextAsync(),
            "previous" => session.TrySkipPreviousAsync(),
            _ => return false,
        };
        op.and_then(|op| op.get()).unwrap_or(false)
    }

    /// Turn a SourceAppUserModelId into something readable. Win32 owners report
    /// an executable (`Spotify.exe`); packaged apps report an AUMID whose last
    /// `!`-segment is the friendliest part. Best-effort, never empty-checks here.
    fn app_name(id: &str) -> String {
        let base = id.rsplit('!').next().unwrap_or(id);
        base.strip_suffix(".exe")
            .or_else(|| base.strip_suffix(".EXE"))
            .unwrap_or(base)
            .to_string()
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
use imp::{current, run_command};

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn current() -> Option<NowPlayingSnapshot> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn run_command(_command: &str, _target: Option<&str>) -> bool {
    false
}
