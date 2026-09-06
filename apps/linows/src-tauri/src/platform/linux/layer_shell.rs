//! The launcher on the wlr-layer-shell overlay layer.
//!
//! An xdg-toplevel cannot sit above a fullscreen window on Wayland: sway and
//! niri paint fullscreen above every layer but `overlay`, and
//! `set_always_on_top` has no protocol behind it. An overlay surface clears
//! fullscreen, takes keyboard focus through `keyboard-interactivity`, and is
//! centred by the compositor when it anchors to no edge.
//!
//! GTK cannot turn a mapped toplevel into a layer surface, so the webview's
//! vbox is reparented into a window of ours, layer-initialised before it is
//! realised. That orphans Tauri's `WebviewWindow`: `show`, `hide`,
//! `is_visible` and `set_focus` on it no longer describe anything on screen,
//! and `set_focus` would map the empty husk. Those callers route here instead.

use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Soname, resolved through the loader's search path. `LOOK_GTK_LAYER_SHELL`
/// lets the Nix package bake in an absolute store path instead, which keeps
/// `LD_LIBRARY_PATH` out of the environment of every app Look launches.
const LIB: &str = "libgtk-layer-shell.so.0";

/// Escape hatch back to the toplevel, for a compositor whose layer-shell
/// implementation misbehaves.
const ENV_DISABLE: &str = "LOOK_NO_LAYER_SHELL";

const NAMESPACE: &std::ffi::CStr = c"lookapp";

/// `GtkLayerShellLayer`. Only overlay clears a fullscreen window.
const LAYER_OVERLAY: u32 = 3;

/// `GtkLayerShellKeyboardMode`. The protocol says a compositor "should give
/// the surface keyboard focus on creation" under on-demand, but sway reads
/// that as click-to-focus: its `arrange_layers` only keeps the keyboard on a
/// layer whose interactivity is exclusive, and drops any other layer's focus
/// the moment it runs. The launcher then maps, blinks a caret, and swallows
/// every key until the user clicks it. Exclusive is what the wlr-layer-shell
/// launchers all use, for exactly this reason.
///
/// The cost is that a click outside no longer dismisses: focus never leaves,
/// so no focus-out arrives to trigger it. Escape and running an entry still
/// do, and both restore the mode below.
const KEYBOARD_NONE: u32 = 0;
const KEYBOARD_EXCLUSIVE: u32 = 1;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static VISIBLE: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// GTK types are not `Send` and every touch of one belongs on the thread
    /// running the main context, so off-thread callers marshal in.
    static LAYER: RefCell<Option<gtk::ApplicationWindow>> = const { RefCell::new(None) };
}

struct Api {
    /// Kept so the library outlives the function pointers taken from it.
    _lib: libloading::Library,
    is_supported: unsafe extern "C" fn() -> i32,
    init_for_window: unsafe extern "C" fn(*mut c_void),
    set_layer: unsafe extern "C" fn(*mut c_void, u32),
    set_keyboard_mode: unsafe extern "C" fn(*mut c_void, u32),
    set_namespace: unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char),
}

/// Opened at runtime rather than linked, so a system without the library falls
/// back to the toplevel path instead of failing to start.
fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let path = option_env!("LOOK_GTK_LAYER_SHELL").unwrap_or(LIB);
        let lib = libloading::Library::new(path)
            .inspect_err(|e| eprintln!("[look:layer-shell] {path} not loadable: {e}"))
            .ok()?;
        let is_supported = *lib.get(b"gtk_layer_is_supported\0").ok()?;
        let init_for_window = *lib.get(b"gtk_layer_init_for_window\0").ok()?;
        let set_layer = *lib.get(b"gtk_layer_set_layer\0").ok()?;
        let set_keyboard_mode = *lib.get(b"gtk_layer_set_keyboard_mode\0").ok()?;
        let set_namespace = *lib.get(b"gtk_layer_set_namespace\0").ok()?;
        Some(Api {
            _lib: lib,
            is_supported,
            init_for_window,
            set_layer,
            set_keyboard_mode,
            set_namespace,
        })
    })
    .as_ref()
}

/// Move the webview onto an overlay layer surface. Main thread only, and
/// before anything shows the Tauri window. `false` leaves every caller on the
/// toplevel path unchanged.
pub fn attach(window: &tauri::WebviewWindow, size: Option<(i32, i32)>) -> bool {
    if super::transparency::window_is_x11() || std::env::var_os(ENV_DISABLE).is_some() {
        return false;
    }
    let Some(api) = api() else {
        return false;
    };
    // False on compositors without the protocol, mutter above all.
    if unsafe { (api.is_supported)() } == 0 {
        eprintln!("[look:layer-shell] compositor does not advertise wlr-layer-shell");
        return false;
    }

    let (Ok(toplevel), Ok(vbox)) = (window.gtk_window(), window.default_vbox()) else {
        return false;
    };
    let Some(app) = toplevel.application() else {
        return false;
    };

    // The husk Tauri still hands out; it must never map again.
    let _ = window.hide();

    let layer = gtk::ApplicationWindow::new(&app);
    layer.set_decorated(false);
    // Without this GTK paints its opaque theme background over the rounded
    // launcher, the same reason apply_transparency exists for the toplevel.
    layer.set_app_paintable(true);
    if let Some(visual) = WidgetExt::screen(&layer).and_then(|screen| screen.rgba_visual()) {
        layer.set_visual(Some(&visual));
    }
    // app_paintable stops GTK drawing the background, but nothing then clears
    // the buffer: wherever the page is transparent an RGBA window shows
    // whatever the surface already held. Proceed so the webview still draws.
    layer.connect_draw(|_, cr| {
        cr.set_operator(gtk::cairo::Operator::Source);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = cr.paint();
        cr.set_operator(gtk::cairo::Operator::Over);
        glib::Propagation::Proceed
    });

    toplevel.remove(&vbox);
    layer.add(&vbox);
    // gtk_container_remove drops the toplevel's focus widget, and the new
    // window inherits none. The compositor still hands the layer surface the
    // keyboard, but GtkWindow has nowhere to route a key press, so the
    // launcher comes up unable to type into.
    if let Some(target) = focus_target(&layer) {
        layer.set_focus(Some(&target));
    }

    let ptr = layer.as_ptr() as *mut c_void;
    unsafe {
        (api.init_for_window)(ptr);
        (api.set_layer)(ptr, LAYER_OVERLAY);
        (api.set_namespace)(ptr, NAMESPACE.as_ptr());
        (api.set_keyboard_mode)(ptr, KEYBOARD_NONE);
    }

    // Anchored nowhere, so the compositor centres it; the size is whatever the
    // widget asks for, and there is no set_position counterpart.
    let (width, height) = usable_size(size.unwrap_or_else(|| fallback_size(window)));
    layer.set_default_size(width, height);
    layer.set_size_request(width, height);

    // Realized, not shown: the GdkWindow and its RGBA visual have to exist
    // before the first summon, or the first frame arrives untransparent.
    layer.realize();

    LAYER.with(|cell| cell.replace(Some(layer)));
    ACTIVE.store(true, Ordering::Relaxed);
    eprintln!("[look:layer-shell] attached to the overlay layer");
    true
}

/// Only for a startup that identified no monitor: `inner_size` reports
/// tauri.conf's default until the compositor configures the window.
fn fallback_size(window: &tauri::WebviewWindow) -> (i32, i32) {
    let scale = window.scale_factor().unwrap_or(1.0);
    window
        .inner_size()
        .map(|size| {
            let logical = size.to_logical::<i32>(scale);
            (logical.width, logical.height)
        })
        .unwrap_or_default()
}

/// A zero in either axis reaches the compositor as "you pick", and an
/// unanchored layer surface then gets handed the whole output - the launcher
/// rendered full-width and top-aligned. Both sources can produce one: the
/// monitor scan may find no monitor, and `inner_size` reads back 0 on a Wayland
/// window the compositor has not configured yet.
fn usable_size((width, height): (i32, i32)) -> (i32, i32) {
    if width > 0 && height > 0 {
        return (width, height);
    }
    eprintln!("[look:layer-shell] no usable size ({width}x{height}), falling back to the default");
    (crate::BASE_W as i32, crate::BASE_H as i32)
}

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Whether the launcher is on screen, or `None` when the toplevel is still the
/// real window and its own `is_visible` is authoritative.
pub fn visible() -> Option<bool> {
    is_active().then(|| VISIBLE.load(Ordering::Relaxed))
}

pub fn show() {
    VISIBLE.store(true, Ordering::Relaxed);
    on_main(|layer| {
        // Before the map: sway reads the committed interactivity in its map
        // handler, and a mode raised afterwards never reaches that path.
        set_keyboard_mode(layer, KEYBOARD_EXCLUSIVE);
        layer.show_all();
        // show_all marks every child visible again, which is when a widget can
        // hold focus; re-grab here so a dismissal that unmapped the surface
        // does not leave the next summon typing into nothing.
        if let Some(target) = focus_target(layer) {
            target.grab_focus();
        }
    });
}

/// The webview, the only child that takes key events. Looked up rather than
/// stored: the vbox is Tauri's and its contents are not ours to assume.
fn focus_target(layer: &gtk::ApplicationWindow) -> Option<gtk::Widget> {
    let vbox = layer.children().into_iter().next()?;
    vbox.downcast::<gtk::Container>()
        .ok()?
        .children()
        .into_iter()
        .find(|child| child.can_focus())
}

fn set_keyboard_mode(layer: &gtk::ApplicationWindow, mode: u32) {
    if let Some(api) = api() {
        unsafe { (api.set_keyboard_mode)(layer.as_ptr() as *mut c_void, mode) }
    }
}

pub fn hide() {
    VISIBLE.store(false, Ordering::Relaxed);
    on_main(|layer| {
        // Give the keyboard back before the surface goes; an exclusive layer
        // that merely unmaps can leave the session without focus.
        set_keyboard_mode(layer, KEYBOARD_NONE);
        layer.hide();
    });
}

/// Keyboard focus arriving and leaving, which `WindowEvent::Focused` no longer
/// reports for a surface Tauri does not own. Main thread only.
pub fn on_focus(handler: impl Fn(bool) + 'static) {
    let handler = std::rc::Rc::new(handler);
    LAYER.with(|cell| {
        let Some(layer) = cell.borrow().clone() else {
            return;
        };
        let entered = handler.clone();
        layer.connect_focus_in_event(move |_, _| {
            entered(true);
            glib::Propagation::Proceed
        });
        layer.connect_focus_out_event(move |_, _| {
            handler(false);
            glib::Propagation::Proceed
        });
    });
}

/// The hotkey arrives on the D-Bus thread; GTK belongs to the main context.
fn on_main(f: impl FnOnce(&gtk::ApplicationWindow) + Send + 'static) {
    if !is_active() {
        return;
    }
    glib::idle_add_once(move || {
        LAYER.with(|cell| {
            if let Some(layer) = cell.borrow().as_ref() {
                f(layer);
            }
        });
    });
}
