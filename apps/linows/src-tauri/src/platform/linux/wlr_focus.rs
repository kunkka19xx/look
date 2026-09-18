//! Focus an existing window via `wlr-foreign-toplevel-management-v1`.
//!
//! Spawn-based "launchers" (`gtk-launch`, `gio launch`, `firefox` itself)
//! cannot focus a running window on Wayland - most apps just open a fresh
//! window when invoked again, regardless of `XDG_ACTIVATION_TOKEN`. The
//! foreign-toplevel-management protocol exposes an explicit `activate(seat)`
//! request that the compositor honours unconditionally, which is the only
//! reliable focus path on Hyprland v0.55 (where the in-tree dispatcher IPC
//! is broken) and other wlroots-based compositors.

use std::time::{Duration, Instant};

use wayland_client::{
    Connection, Dispatch, QueueHandle, event_created_child,
    protocol::{wl_registry, wl_seat},
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self as wlr_toplevel, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self as wlr_manager, ZwlrForeignToplevelManagerV1},
};

/// `zwlr_foreign_toplevel_handle_v1.state.activated`.
const ACTIVATED_STATE: u32 = 2;

/// The `app_id` of the toplevel the compositor has activated, which is the app
/// the user is in. Names a copied image after where it came from.
pub fn focused_app_id() -> Option<String> {
    let state = collect_toplevels()?;
    state
        .toplevels
        .iter()
        .find(|t| t.activated)
        .and_then(|t| t.app_id.clone())
}

/// Collect app_ids of all visible toplevels on wlroots-based compositors.
/// Returns an empty set if the protocol isn't available.
pub fn list_toplevel_app_ids() -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    let Some(state) = collect_toplevels() else {
        return ids;
    };
    for tl in &state.toplevels {
        if let Some(id) = &tl.app_id {
            ids.insert(id.to_lowercase());
        }
    }
    ids
}

/// Every window the compositor manages, with the app_id and activated state it
/// reported.
fn collect_toplevels() -> Option<State> {
    let conn = Connection::connect_to_env().ok()?;
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    let mut state = State {
        target: String::new(),
        seat: None,
        manager_bound: false,
        toplevels: Vec::new(),
    };

    if queue.roundtrip(&mut state).is_err() || !state.manager_bound {
        return None;
    }
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if !state.toplevels.is_empty() && state.toplevels.iter().all(|t| t.done) {
            break;
        }
        if queue.roundtrip(&mut state).is_err() {
            break;
        }
    }
    Some(state)
}

/// Try to activate an existing toplevel whose `app_id` matches (case-insensitive).
/// Returns true if a match was found and the activate request was sent.
pub fn try_focus(app_id: &str) -> bool {
    let Ok(conn) = Connection::connect_to_env() else {
        eprintln!("[wlr-focus] failed to open Wayland connection");
        return false;
    };
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    let mut state = State {
        target: app_id.to_lowercase(),
        seat: None,
        manager_bound: false,
        toplevels: Vec::new(),
    };

    // First roundtrip: process registry → bind manager + seat.
    if queue.roundtrip(&mut state).is_err() {
        return false;
    }
    if !state.manager_bound {
        eprintln!("[wlr-focus] compositor doesn't advertise zwlr_foreign_toplevel_manager_v1");
        return false;
    }

    // Pump events until every toplevel reports Done, or the deadline trips.
    // The compositor enumerates existing toplevels asynchronously after the
    // manager binds; each toplevel sends its initial state (incl. app_id),
    // terminated by a Done event.
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if !state.toplevels.is_empty() && state.toplevels.iter().all(|t| t.done) {
            break;
        }
        if queue.roundtrip(&mut state).is_err() {
            break;
        }
    }

    let Some(seat) = state.seat.clone() else {
        eprintln!("[wlr-focus] no wl_seat available");
        return false;
    };
    let mut activated = false;
    for tl in &state.toplevels {
        if let Some(id) = &tl.app_id
            && id.eq_ignore_ascii_case(&state.target)
        {
            tl.handle.activate(&seat);
            activated = true;
            eprintln!("[wlr-focus] activated toplevel app_id={id}");
            break;
        }
    }
    if activated {
        let _ = conn.flush();
        let _ = queue.roundtrip(&mut state);
    } else {
        eprintln!(
            "[wlr-focus] no toplevel matched app_id={} (scanned {} windows)",
            state.target,
            state.toplevels.len()
        );
    }
    activated
}

struct ToplevelEntry {
    handle: ZwlrForeignToplevelHandleV1,
    app_id: Option<String>,
    /// The compositor's own word for "this is the window with focus".
    activated: bool,
    done: bool,
}

struct State {
    target: String,
    seat: Option<wl_seat::WlSeat>,
    manager_bound: bool,
    toplevels: Vec<ToplevelEntry>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_seat" => {
                let seat: wl_seat::WlSeat = registry.bind(name, version.min(7), qh, ());
                state.seat = Some(seat);
            }
            "zwlr_foreign_toplevel_manager_v1" => {
                let _: ZwlrForeignToplevelManagerV1 = registry.bind(name, version.min(3), qh, ());
                state.manager_bound = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: wlr_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wlr_manager::Event::Toplevel { toplevel } = event {
            state.toplevels.push(ToplevelEntry {
                handle: toplevel,
                app_id: None,
                activated: false,
                done: false,
            });
        }
    }

    // The Toplevel event (opcode 0) carries a freshly-created
    // ZwlrForeignToplevelHandleV1 proxy; wayland-client needs to know which
    // user-data type to attach so subsequent handle events dispatch into our
    // Dispatch<ZwlrForeignToplevelHandleV1, ()> impl.
    event_created_child!(State, ZwlrForeignToplevelManagerV1, [
        0 => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrForeignToplevelHandleV1,
        event: wlr_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(entry) = state.toplevels.iter_mut().find(|t| &t.handle == proxy) else {
            return;
        };
        match event {
            wlr_toplevel::Event::AppId { app_id } => entry.app_id = Some(app_id),
            // A packed array of u32s, re-sent in full on every change.
            wlr_toplevel::Event::State { state } => {
                entry.activated = state
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|word| u32::from_ne_bytes(*word) == ACTIVATED_STATE);
            }
            wlr_toplevel::Event::Done => entry.done = true,
            wlr_toplevel::Event::Closed => entry.app_id = None,
            _ => {}
        }
    }
}
