//! Behind-window blur on Wayland, over whichever protocol the compositor
//! advertises: `ext-background-effect-v1` (KWin 6.7+, Mutter 51+, Hyprland
//! 0.56+, Niri) or `org_kde_kwin_blur`, which Plasma spoke until 6.7.
//!
//! Binds against GTK's own `wl_surface` - a second connection cannot address
//! it - so the pointers come from the window handle rather than
//! `connect_to_env` the way `wlr_focus` does. A private event queue keeps our
//! roundtrips from eating the events GDK is waiting for.

use crate::platform::BlurRect;
use std::sync::{Mutex, OnceLock};
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
    effect_manager: Option<ExtBackgroundEffectManagerV1>,
    kde_manager: Option<OrgKdeKwinBlurManager>,
    attached: Mutex<Attached>,
}

/// Per-surface state. GTK3 destroys its `wl_surface` on every hide and makes a
/// new one on show, and `ext` raises `surface_destroyed` (a fatal protocol
/// error) at a region sent to a dead one, so the objects live only while the
/// window is mapped. The region outlives them: the frontend sends it only when
/// it changes, and a re-shown window needs it again.
#[derive(Default)]
struct Attached {
    /// Kept for `org_kde_kwin_blur`, whose object comes and goes with the
    /// region (see `apply`).
    surface: Option<WlSurface>,
    effect: Option<ExtBackgroundEffectSurfaceV1>,
    kde: Option<OrgKdeKwinBlur>,
    rects: Vec<BlurRect>,
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

/// Bind the managers. Call once, from the main thread. A compositor with
/// neither protocol records "unsupported" and no-ops after.
pub fn init(display: *mut std::ffi::c_void) {
    let _ = BLUR.set(bind(display));
}

pub fn is_supported() -> bool {
    BLUR.get()
        .and_then(|b| b.as_ref())
        .is_some_and(|b| b.effect_manager.is_some() || b.kde_manager.is_some())
}

/// Bind to the window's current surface and replay the region. Call on every
/// map, from the main thread, with GTK's live `wl_surface`.
pub fn attach(surface: *mut std::ffi::c_void) {
    with_attached(|blur, attached| {
        release(attached);
        if surface.is_null() {
            return;
        }
        // SAFETY: the pointer is GTK's live wl_surface for this window.
        let Ok(id) = (unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.cast()) }) else {
            return;
        };
        let Ok(wl_surface) = WlSurface::from_id(&blur.conn, id) else {
            return;
        };
        attached.effect = blur
            .effect_manager
            .as_ref()
            .map(|manager| manager.get_background_effect(&wl_surface, &blur.queue_handle, ()));
        attached.surface = Some(wl_surface);
        apply(blur, attached);
    });
}

/// Drop the per-surface objects. Call on unmap; destroying them is legal even
/// after GTK has destroyed the surface under them.
pub fn detach() {
    with_attached(|blur, attached| {
        release(attached);
        let _ = blur.conn.flush();
    });
}

/// Ask for `rects` to be blurred. Remembered while the window is hidden and
/// sent on the next map.
pub fn set_region(rects: &[BlurRect]) {
    with_attached(|blur, attached| {
        attached.rects = rects.to_vec();
        apply(blur, attached);
    });
}

/// A no-op on a compositor without blur. A poisoned lock is still usable: the
/// state is a few handles and a region, left whole by any panic.
fn with_attached(f: impl FnOnce(&Blur, &mut Attached)) {
    let Some(Some(blur)) = BLUR.get() else {
        return;
    };
    f(
        blur,
        &mut blur.attached.lock().unwrap_or_else(|e| e.into_inner()),
    );
}

fn release(attached: &mut Attached) {
    attached.surface = None;
    if let Some(effect) = attached.effect.take() {
        effect.destroy();
    }
    if let Some(kde) = attached.kde.take() {
        kde.release();
    }
}

/// Both protocols copy the region, so it is built, handed over and destroyed
/// in one pass.
///
/// The surface is deliberately not committed: that is GTK's buffer, and
/// committing from here could publish a frame it is still assembling. Both
/// protocols apply on the next surface commit, and every caller is a map or a
/// layout change that repaints anyway.
///
/// The legacy KDE protocol reads an empty region as "blur the whole window", so
/// nothing to blur is said there by unsetting, and the object is created again
/// when a region returns. `ext`'s empty region already means no blur.
fn apply(blur: &Blur, attached: &mut Attached) {
    if let (Some(manager), Some(surface)) = (&blur.kde_manager, &attached.surface) {
        if attached.rects.is_empty() {
            if let Some(kde) = attached.kde.take() {
                manager.unset(surface);
                kde.release();
            }
        } else if attached.kde.is_none() {
            attached.kde = Some(manager.create(surface, &blur.queue_handle, ()));
        }
    }
    if attached.effect.is_none() && attached.kde.is_none() {
        let _ = blur.conn.flush();
        return;
    }
    let region = blur.compositor.create_region(&blur.queue_handle, ());
    for rect in &attached.rects {
        region.add(rect.x, rect.y, rect.width as i32, rect.height as i32);
    }
    if let Some(effect) = &attached.effect {
        effect.set_blur_region(Some(&region));
    }
    if let Some(kde) = &attached.kde {
        kde.set_region(Some(&region));
        kde.commit();
    }
    region.destroy();
    let _ = blur.conn.flush();
}

fn bind(display: *mut std::ffi::c_void) -> Option<Blur> {
    if display.is_null() {
        return None;
    }
    // SAFETY: the pointer comes from a live window's handle, and the backend
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
    let effect_manager = globals.effect_manager.filter(|_| globals.blur_capable);
    let kde_manager = globals.kde_manager;
    if effect_manager.is_none() && kde_manager.is_none() {
        return None;
    }
    // The only visible answer: the effect is the compositor's to draw, so a
    // report otherwise cannot tell "unsupported" from "supported but broken".
    eprintln!(
        "look: compositor blur via {}",
        if effect_manager.is_some() {
            "ext-background-effect-v1"
        } else {
            "org_kde_kwin_blur"
        }
    );

    Some(Blur {
        conn,
        queue_handle,
        compositor,
        effect_manager,
        kde_manager,
        attached: Mutex::default(),
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
