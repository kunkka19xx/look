//! Owning the clipboard through GTK, so one copy offers every form at once.
//! The forms themselves are the backend's; this is only the grab.

use gtk::gdk;
use gtk::glib::translate::ToGlibPtr;
use gtk::{TargetEntry, TargetFlags};
use linows_backend::host::ClipForm;

/// Bits per unit of the payload: bytes, for every type here.
const BYTE_FORMAT: i32 = 8;

/// How long a caller off the main thread waits for the grab's answer before
/// treating it as failed and shelling out.
const GRAB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether the grab took, so a failed one still reaches the shell fallback.
///
/// Every GTK call belongs to the main thread. A sync Tauri command already
/// answers there, so the usual path runs [`grab`] outright; a caller from
/// anywhere else queues it and waits for the answer.
pub fn own_clipboard(app: &tauri::AppHandle, forms: Vec<ClipForm>) -> bool {
    if gtk::is_initialized_main_thread() {
        return grab(forms);
    }

    let (answer, wait) = std::sync::mpsc::sync_channel(1);
    let queued = app.run_on_main_thread(move || {
        let _ = answer.send(grab(forms));
    });
    if queued.is_err() {
        return false;
    }
    wait.recv_timeout(GRAB_TIMEOUT).unwrap_or(false)
}

/// Hand the payloads to GTK and become the clipboard owner. Main thread only.
/// A target reaches the getter as the number it was registered under, which
/// here is its form's position in the list.
fn grab(forms: Vec<ClipForm>) -> bool {
    let Some(display) = gdk::Display::default() else {
        return false;
    };
    let clipboard = gtk::Clipboard::for_display(&display, &gdk::SELECTION_CLIPBOARD);

    let targets: Vec<TargetEntry> = forms
        .iter()
        .enumerate()
        .flat_map(|(index, form)| {
            form.targets
                .iter()
                .map(move |target| TargetEntry::new(target, TargetFlags::empty(), index as u32))
        })
        .collect();

    // Answered on demand, once per paste, for as long as Look holds the
    // clipboard, which is why the payloads are moved in rather than borrowed.
    let owned = clipboard.set_with_data(&targets, move |_, selection, info| {
        let Some(form) = forms.get(info as usize) else {
            return;
        };
        selection.set(&selection.target(), BYTE_FORMAT, &form.payload);
    });

    if owned {
        allow_manager_to_store(&clipboard);
    }
    owned
}

/// Offer the content to the desktop's clipboard manager, so a copy outlives
/// Look the way the forked `wl-copy` used to.
///
/// `set_can_store` is not bound in gtk-rs; a null target list is GTK's own
/// spelling of "every form currently set is storable".
fn allow_manager_to_store(clipboard: &gtk::Clipboard) {
    unsafe {
        gtk::ffi::gtk_clipboard_set_can_store(clipboard.to_glib_none().0, std::ptr::null(), 0);
    }
    clipboard.store();
}
