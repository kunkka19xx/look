//! The first output's logical size, over `zxdg_output_manager_v1`.
//! `wl_output` reports a whole-number scale, so a 1.8 panel would read as 2.

use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_output::WlOutput, wl_registry},
};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1,
    zxdg_output_v1::{self, ZxdgOutputV1},
};

/// Logical height of the first output; `None` off Wayland or without xdg-output.
pub fn logical_height() -> Option<u32> {
    let conn = Connection::connect_to_env().ok()?;
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut state = State::default();
    queue.roundtrip(&mut state).ok()?;
    let manager = state.manager.take()?;
    manager.get_xdg_output(state.output.as_ref()?, &qh, ());
    queue.roundtrip(&mut state).ok()?;
    state.height.filter(|&h| h > 0)
}

#[derive(Default)]
struct State {
    manager: Option<ZxdgOutputManagerV1>,
    output: Option<WlOutput>,
    height: Option<u32>,
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
            "wl_output" if state.output.is_none() => {
                state.output = Some(registry.bind(name, version.min(4), qh, ()));
            }
            "zxdg_output_manager_v1" => {
                state.manager = Some(registry.bind(name, version.min(3), qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<ZxdgOutputV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_output_v1::Event::LogicalSize { height, .. } = event {
            state.height = u32::try_from(height).ok();
        }
    }
}

wayland_client::delegate_noop!(State: ignore WlOutput);
wayland_client::delegate_noop!(State: ignore ZxdgOutputManagerV1);
