//! Which window has focus on KDE Plasma, over the protocol Plasma's own task
//! manager speaks. KWin offers neither the wlroots toplevel protocol nor a
//! D-Bus call for this, so nothing else names the focused app there.

use std::time::{Duration, Instant};

use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, protocol::wl_registry};
use wayland_protocols_plasma::plasma_window_management::client::{
    org_kde_plasma_window::{self as plasma_window, OrgKdePlasmaWindow},
    org_kde_plasma_window_management::{self as plasma_management, OrgKdePlasmaWindowManagement},
};

/// Where `window_with_uuid` arrived. `get_window_by_uuid` landed a version
/// earlier, but without the event nothing announces a window to ask about,
/// and the integer ids of the older path are not worth a second code path.
const UUID_VERSION: u32 = 13;
/// How long KWin gets to enumerate its windows before the answer goes without
/// them.
const ENUMERATE_TIMEOUT: Duration = Duration::from_millis(500);

/// Whether KWin is there and speaks a new enough window management.
pub fn available() -> bool {
    bind().is_some()
}

/// The `app_id` of the window KWin has marked active.
pub fn focused_app_id() -> Option<String> {
    let (_conn, mut queue, mut state) = bind()?;

    // KWin announces its windows after the manager binds, and each one sends
    // its properties before saying it is done.
    let deadline = Instant::now() + ENUMERATE_TIMEOUT;
    while Instant::now() < deadline {
        if !state.windows.is_empty() && state.windows.iter().all(|w| w.settled) {
            break;
        }
        if queue.roundtrip(&mut state).is_err() {
            break;
        }
    }

    state
        .windows
        .iter()
        .find(|w| w.active)
        .and_then(|w| w.app_id.clone())
}

fn bind() -> Option<(Connection, EventQueue<State>, State)> {
    let conn = Connection::connect_to_env().ok()?;
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    let mut state = State {
        manager_bound: false,
        windows: Vec::new(),
    };
    queue.roundtrip(&mut state).ok()?;
    state.manager_bound.then_some((conn, queue, state))
}

struct WindowEntry {
    handle: OrgKdePlasmaWindow,
    app_id: Option<String>,
    /// KWin's own word for "this is the window with focus".
    active: bool,
    settled: bool,
}

struct State {
    manager_bound: bool,
    windows: Vec<WindowEntry>,
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
        let manager = OrgKdePlasmaWindowManagement::interface();
        if interface != manager.name || version < UUID_VERSION {
            return;
        }
        let _: OrgKdePlasmaWindowManagement =
            registry.bind(name, version.min(manager.version), qh, ());
        state.manager_bound = true;
    }
}

impl Dispatch<OrgKdePlasmaWindowManagement, ()> for State {
    fn event(
        state: &mut Self,
        manager: &OrgKdePlasmaWindowManagement,
        event: plasma_management::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let plasma_management::Event::WindowWithUuid { uuid, .. } = event else {
            return;
        };
        state.windows.push(WindowEntry {
            handle: manager.get_window_by_uuid(uuid, qh, ()),
            app_id: None,
            active: false,
            settled: false,
        });
    }
}

impl Dispatch<OrgKdePlasmaWindow, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &OrgKdePlasmaWindow,
        event: plasma_window::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(entry) = state.windows.iter_mut().find(|w| &w.handle == proxy) else {
            return;
        };
        match event {
            plasma_window::Event::AppIdChanged { app_id } => entry.app_id = Some(app_id),
            plasma_window::Event::StateChanged { flags } => {
                entry.active = flags & plasma_management::State::Active as u32 != 0;
            }
            plasma_window::Event::InitialState => entry.settled = true,
            plasma_window::Event::Unmapped => {
                entry.active = false;
                entry.settled = true;
            }
            _ => {}
        }
    }
}
