//! Behind-window blur on Wayland, over whichever protocol the compositor
//! advertises: `ext-background-effect-v1` (KWin 6.7+, Hyprland 0.56+, Niri) or
//! `org_kde_kwin_blur`, which Plasma spoke until 6.7.
//!
//! Binds against GTK's own `wl_surface` - a second connection cannot address
//! it - so the pointers come from the window handle rather than
//! `connect_to_env` the way `wlr_focus` does. A private event queue keeps our
//! roundtrips from eating the events GDK is waiting for.

use crate::platform::BlurRect;
use std::sync::OnceLock;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::{
    wl_compositor::WlCompositor, wl_region::WlRegion, wl_registry, wl_surface::WlSurface,
};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::{self, ExtBackgroundEffectManagerV1},
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use wayland_protocols_plasma::blur::client::{
    org_kde_kwin_blur::OrgKdeKwinBlur, org_kde_kwin_blur_manager::OrgKdeKwinBlurManager,
};

/// Bound once: rebinding per call would mean a registry roundtrip every time a
/// pane resizes.
static BLUR: OnceLock<Option<Blur>> = OnceLock::new();

struct Blur {
    conn: Connection,
    queue_handle: QueueHandle<Globals>,
    compositor: WlCompositor,
    effect: Option<ExtBackgroundEffectSurfaceV1>,
    kde: Option<OrgKdeKwinBlur>,
}

#[derive(Default)]
struct Globals {
    compositor: Option<WlCompositor>,
    effect_manager: Option<ExtBackgroundEffectManagerV1>,
    kde_manager: Option<OrgKdeKwinBlurManager>,
    /// `ext` advertises what it can do; a manager that never claims blur is
    /// not support.
    blur_capable: bool,
}

/// Bind against the window's surface. Call once, from the main thread. A
/// compositor with neither protocol records "unsupported" and no-ops after.
pub fn init(display: *mut std::ffi::c_void, surface: *mut std::ffi::c_void) {
    let _ = BLUR.set(bind(display, surface));
}

pub fn is_supported() -> bool {
    BLUR.get()
        .and_then(|b| b.as_ref())
        .is_some_and(|b| b.effect.is_some() || b.kde.is_some())
}

/// Ask for `rects` to be blurred. Both protocols copy the region, so it is
/// built, handed over and destroyed in one pass.
///
/// The surface is deliberately not committed: that is GTK's buffer, and
/// committing from this thread could publish a frame it is still assembling.
/// Both protocols apply on the next surface commit, and every caller is a
/// layout change that repaints anyway.
pub fn set_region(rects: &[BlurRect]) {
    let Some(Some(blur)) = BLUR.get() else {
        return;
    };
    let region = blur.compositor.create_region(&blur.queue_handle, ());
    for rect in rects {
        region.add(rect.x, rect.y, rect.width as i32, rect.height as i32);
    }
    if let Some(effect) = &blur.effect {
        effect.set_blur_region(Some(&region));
    }
    if let Some(kde) = &blur.kde {
        kde.set_region(Some(&region));
        kde.commit();
    }
    region.destroy();
    let _ = blur.conn.flush();
}

fn bind(display: *mut std::ffi::c_void, surface: *mut std::ffi::c_void) -> Option<Blur> {
    if display.is_null() || surface.is_null() {
        return None;
    }
    // SAFETY: pointers come from a live window's handle, and the backend
    // borrows the display - GDK keeps driving the same connection.
    let backend = unsafe { Backend::from_foreign_display(display.cast()) };
    let conn = Connection::from_backend(backend);

    let mut queue = conn.new_event_queue();
    let queue_handle = queue.handle();
    let mut globals = Globals::default();
    conn.display().get_registry(&queue_handle, ());
    // Two passes: globals, then the capabilities event that binding triggers.
    queue.roundtrip(&mut globals).ok()?;
    queue.roundtrip(&mut globals).ok()?;

    let compositor = globals.compositor?;
    // SAFETY: the pointer is GTK's live wl_surface for this window.
    let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.cast()) }.ok()?;
    let wl_surface = WlSurface::from_id(&conn, id).ok()?;

    let effect = globals
        .effect_manager
        .filter(|_| globals.blur_capable)
        .map(|manager| manager.get_background_effect(&wl_surface, &queue_handle, ()));
    let kde = globals
        .kde_manager
        .map(|manager| manager.create(&wl_surface, &queue_handle, ()));

    if effect.is_none() && kde.is_none() {
        return None;
    }
    // The only visible answer: the effect is the compositor's to draw, so a
    // report otherwise cannot tell "unsupported" from "supported but broken".
    eprintln!(
        "look: compositor blur via {}",
        if effect.is_some() {
            "ext-background-effect-v1"
        } else {
            "org_kde_kwin_blur"
        }
    );
    let _ = conn.flush();

    Some(Blur {
        conn,
        queue_handle,
        compositor,
        effect,
        kde,
    })
}

impl Dispatch<wl_registry::WlRegistry, ()> for Globals {
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
            "wl_compositor" => {
                state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
            }
            "ext_background_effect_manager_v1" => {
                state.effect_manager = Some(registry.bind(name, 1, qh, ()));
            }
            "org_kde_kwin_blur_manager" => {
                state.kde_manager = Some(registry.bind(name, 1, qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for Globals {
    fn event(
        state: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        event: ext_background_effect_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_background_effect_manager_v1::Event::Capabilities { flags } = event {
            let blur = ext_background_effect_manager_v1::Capability::Blur;
            state.blur_capable = match flags {
                wayland_client::WEnum::Value(value) => value.contains(blur),
                wayland_client::WEnum::Unknown(bits) => bits & blur.bits() != 0,
            };
        }
    }
}

// Event-less objects; these exist only to satisfy the dispatch bound.
macro_rules! ignore_events {
    ($($proxy:ty),+ $(,)?) => {$(
        impl Dispatch<$proxy, ()> for Globals {
            fn event(
                _: &mut Self,
                _: &$proxy,
                _: <$proxy as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    )+};
}

ignore_events!(
    WlCompositor,
    WlRegion,
    WlSurface,
    ExtBackgroundEffectSurfaceV1,
    OrgKdeKwinBlurManager,
    OrgKdeKwinBlur,
);
