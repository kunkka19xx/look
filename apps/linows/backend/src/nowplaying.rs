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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

/// The current track, or `None` when nothing is playing. Blocks on the D-Bus
/// read, so the shell runs it off the UI thread.
pub fn now_playing_current() -> Option<NowPlayingSnapshot> {
    current()
}

/// A live subscription to the current track. `on_change` runs once with
/// the first read and again after every change a player signals, on a
/// thread of its own; between changes nothing wakes. Dropping the watch
/// stops the thread.
pub struct NowPlayingWatch {
    _stop: imp::Stop,
}

type OnChange = Box<dyn Fn(Option<NowPlayingSnapshot>) + Send>;

pub fn now_playing_watch(
    on_change: impl Fn(Option<NowPlayingSnapshot>) + Send + 'static,
) -> NowPlayingWatch {
    NowPlayingWatch {
        _stop: imp::watch(Box::new(on_change)),
    }
}

/// Send a transport command (`"playpause"`, `"next"`, `"previous"`) to a player.
/// `player` is the handle from the snapshot being controlled; when omitted the
/// active player is re-picked. Returns whether it was delivered.
pub fn now_playing_command(command: &str, player: Option<&str>) -> bool {
    run_command(command, player)
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{NowPlayingSnapshot, OnChange};
    use crate::platform::linux::dbus;
    use futures_util::future::{Either, join, join_all, select};
    use futures_util::{FutureExt, StreamExt};
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::oneshot;
    use zbus::message::Type;
    use zbus::zvariant::{OwnedValue, Str, Value};
    use zbus::{MatchRule, MessageStream};

    const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
    const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
    const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
    const ROOT_IFACE: &str = "org.mpris.MediaPlayer2";
    const PLAYING: &str = "Playing";
    const PROPERTIES_IFACE: &str = "org.freedesktop.DBus.Properties";
    const PROPERTIES_CHANGED: &str = "PropertiesChanged";
    const BUS_NAME: &str = "org.freedesktop.DBus";
    const NAME_OWNER_CHANGED: &str = "NameOwnerChanged";
    /// The player properties the tile shows; a change to any other one
    /// (volume, rate, loop) is not worth a read.
    const WATCHED: [&str; 2] = ["PlaybackStatus", "Metadata"];
    /// Signals queued per stream before the bus drops the oldest.
    const SIGNAL_QUEUE: usize = 16;
    /// A player that changes track emits a burst; it collapses into one read.
    const COALESCE: Duration = Duration::from_millis(50);
    /// Some MPRIS bridges never signal, so the state is re-read this often.
    const SAFETY_POLL: Duration = Duration::from_secs(30);

    /// Closing it, or dropping it, ends the watch thread.
    pub type Stop = oneshot::Sender<()>;

    pub fn watch(on_change: OnChange) -> Stop {
        let (stop, stopped) = oneshot::channel();
        // A failed spawn leaves the tile on its last snapshot.
        let _ = std::thread::Builder::new()
            .name("look-mpris".into())
            .spawn(move || {
                // Its own runtime: the shared one serves one blocking call
                // at a time, and this loop never returns.
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(run(on_change, stopped)),
                    Err(_) => on_change(None),
                }
            });
        stop
    }

    async fn run(on_change: OnChange, mut stopped: oneshot::Receiver<()>) {
        let Some(conn) = dbus::session() else {
            on_change(None);
            return;
        };
        // Without signals the safety poll still carries the tile.
        let mut events = subscribe(conn)
            .await
            .unwrap_or_else(|| Box::pin(futures_util::stream::pending()));
        let mut last = current_async(conn).await;
        on_change(last.clone());
        loop {
            match tokio::time::timeout(SAFETY_POLL, select(events.next(), &mut stopped)).await {
                Ok(Either::Right(_)) | Ok(Either::Left((None, _))) => return,
                Ok(Either::Left((Some(event), _))) => {
                    if !relevant(&event) {
                        continue;
                    }
                    tokio::time::sleep(COALESCE).await;
                    while let Some(Some(_)) = events.next().now_or_never() {}
                }
                Err(_) => {}
            }
            let snapshot = current_async(conn).await;
            if snapshot != last {
                last = snapshot.clone();
                on_change(snapshot);
            }
        }
    }

    type Events = Pin<Box<dyn futures_util::Stream<Item = zbus::Message>>>;

    /// Property changes on any player's object, and MPRIS names coming and
    /// going on the bus. Both match rules leave the bus with the streams.
    async fn subscribe(conn: &zbus::Connection) -> Option<Events> {
        let properties = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(PROPERTIES_IFACE)
            .ok()?
            .member(PROPERTIES_CHANGED)
            .ok()?
            .path(MPRIS_PATH)
            .ok()?
            .build();
        let names = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(BUS_NAME)
            .ok()?
            .interface(BUS_NAME)
            .ok()?
            .member(NAME_OWNER_CHANGED)
            .ok()?
            .arg0ns(MPRIS_PREFIX.trim_end_matches('.'))
            .ok()?
            .build();
        let properties = MessageStream::for_match_rule(properties, conn, Some(SIGNAL_QUEUE))
            .await
            .ok()?;
        let names = MessageStream::for_match_rule(names, conn, Some(SIGNAL_QUEUE))
            .await
            .ok()?;
        let events = futures_util::stream::select(properties, names)
            .filter_map(|message| futures_util::future::ready(message.ok()));
        Some(Box::pin(events))
    }

    /// Whether a signal can have changed what the tile shows.
    fn relevant(message: &zbus::Message) -> bool {
        let header = message.header();
        let Some(member) = header.member().map(|m| m.as_str()) else {
            return false;
        };
        let body = message.body();
        if member == PROPERTIES_CHANGED {
            let Ok((iface, changed, invalidated)) =
                body.deserialize::<(&str, HashMap<&str, Value>, Vec<&str>)>()
            else {
                return false;
            };
            return relevant_change(iface, changed.keys().copied(), &invalidated);
        }
        if member == NAME_OWNER_CHANGED {
            return body
                .deserialize::<(&str, &str, &str)>()
                .is_ok_and(|(name, _, _)| is_player(name));
        }
        false
    }

    /// A player interface change that touches the status or the track.
    fn relevant_change<'a>(
        iface: &str,
        changed: impl Iterator<Item = &'a str>,
        invalidated: &[&'a str],
    ) -> bool {
        iface == PLAYER_IFACE
            && changed
                .chain(invalidated.iter().copied())
                .any(|key| WATCHED.contains(&key))
    }

    fn is_player(name: &str) -> bool {
        name.starts_with(MPRIS_PREFIX)
    }

    /// The player the tile last locked onto, so a background player that starts
    /// later can't steal it while the current one is still playing.
    static LAST_ACTIVE: Mutex<Option<String>> = Mutex::new(None);

    pub fn current() -> Option<NowPlayingSnapshot> {
        let conn = dbus::session()?;
        dbus::block_on(current_async(conn))
    }

    async fn current_async(conn: &zbus::Connection) -> Option<NowPlayingSnapshot> {
        let (name, status) = active_player(conn).await?;
        read_snapshot(conn, &name, &status).await
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
        dbus::block_on(async {
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

        let states = join_all(names.into_iter().filter(|name| is_player(name)).map(
            |name| async move {
                let state = player_state(conn, &name).await;
                (name, state)
            },
        ))
        .await;
        let candidates: Vec<(String, String)> = states
            .into_iter()
            .filter_map(|(name, state)| match state {
                Some((status, true)) => Some((name, status)),
                _ => None,
            })
            .collect();
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
        let (status, can_control) = join(
            proxy.get_property::<String>("PlaybackStatus"),
            proxy.get_property::<bool>("CanControl"),
        )
        .await;
        Some((status.ok()?, can_control.unwrap_or(true)))
    }

    async fn read_snapshot(
        conn: &zbus::Connection,
        name: &str,
        status: &str,
    ) -> Option<NowPlayingSnapshot> {
        let (player, root) = join(
            player_proxy(conn, name, PLAYER_IFACE),
            player_proxy(conn, name, ROOT_IFACE),
        )
        .await;
        // A missing app name just leaves `app` None; it must not blank the tile.
        let (metadata, app) = join(
            player?.get_property::<HashMap<String, OwnedValue>>("Metadata"),
            async {
                match root {
                    Some(proxy) => proxy.get_property::<String>("Identity").await.ok(),
                    None => None,
                }
            },
        )
        .await;
        let metadata = metadata.ok()?;

        let title = metadata.get("xesam:title").and_then(one_string)?;
        if title.is_empty() {
            return None;
        }
        let artist = metadata
            .get("xesam:artist")
            .and_then(joined_strings)
            .filter(|s| !s.is_empty());

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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn only_status_and_track_changes_on_the_player_count() {
            assert!(relevant_change(
                PLAYER_IFACE,
                ["PlaybackStatus"].into_iter(),
                &[]
            ));
            assert!(relevant_change(
                PLAYER_IFACE,
                ["Volume", "Metadata"].into_iter(),
                &[]
            ));
            assert!(relevant_change(PLAYER_IFACE, [].into_iter(), &["Metadata"]));
            assert!(!relevant_change(PLAYER_IFACE, ["Volume"].into_iter(), &[]));
            assert!(!relevant_change(
                ROOT_IFACE,
                ["PlaybackStatus"].into_iter(),
                &[]
            ));
        }

        #[test]
        fn only_mpris_names_count() {
            assert!(is_player("org.mpris.MediaPlayer2.spotify"));
            assert!(is_player("org.mpris.MediaPlayer2.firefox.instance_1_23"));
            assert!(!is_player("org.freedesktop.Notifications"));
        }
    }
}

/// Now Playing via the System Media Transport Controls (SMTC), the Windows peer
/// of MPRIS. `GlobalSystemMediaTransportControlsSessionManager` surfaces whatever
/// app currently owns the media keys (browsers, Spotify, Groove, ...), so a
/// glance and the transport work without per-app integration.
#[cfg(target_os = "windows")]
mod imp {
    use super::{NowPlayingSnapshot, OnChange};
    use crate::platform::windows::ensure_mta;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;
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

    /// Until SMTC events are wired, the watch is a poll on its own thread
    /// that reports only changes.
    const POLL: Duration = Duration::from_secs(2);

    /// Dropping it ends the poll thread.
    pub type Stop = mpsc::Sender<()>;

    pub fn watch(on_change: OnChange) -> Stop {
        let (stop, stopped) = mpsc::channel::<()>();
        let _ = std::thread::Builder::new()
            .name("look-smtc".into())
            .spawn(move || {
                let mut last = current();
                on_change(last.clone());
                while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(POLL) {
                    let now = current();
                    if now != last {
                        last = now.clone();
                        on_change(now);
                    }
                }
            });
        stop
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

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod imp {
    use super::{NowPlayingSnapshot, OnChange};

    pub type Stop = ();

    pub fn watch(on_change: OnChange) -> Stop {
        on_change(None);
    }

    pub fn current() -> Option<NowPlayingSnapshot> {
        None
    }

    pub fn run_command(_command: &str, _target: Option<&str>) -> bool {
        false
    }
}

use imp::{current, run_command};
