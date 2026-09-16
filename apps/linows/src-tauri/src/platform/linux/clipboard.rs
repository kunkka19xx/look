//! Linux file clipboard: one copy that a file manager pastes as a file and a
//! text field pastes as a path.
//!
//! A clipboard advertises a *list* of types and the pasting app asks for the
//! one it understands, which is how macOS writes a URL and a string in a single
//! copy (`NSPasteboard.writeObjects([url, path])`). `wl-copy` and `xclip` each
//! advertise one type per invocation, so Look owns the clipboard itself through
//! GTK and answers whichever type is requested. They stay as the fallback for
//! the case where the grab fails.

use std::io::Write;
use std::process::Stdio;

use gtk::gdk;
use gtk::glib::translate::ToGlibPtr;
use gtk::{TargetEntry, TargetFlags};

/// What the GNOME family asks for: a verb, then one URI per line.
const GNOME_COPIED_FILES: &str = "x-special/gnome-copied-files";
/// What KDE and the XFCE family ask for.
const URI_LIST: &str = "text/uri-list";
/// Every spelling a text widget might ask for. Offered after the file forms,
/// matching the order macOS writes them in: the richer representation first.
const TEXT_TARGETS: [&str; 4] = [
    "text/plain;charset=utf-8",
    "text/plain",
    "UTF8_STRING",
    "STRING",
];

/// What a copied image is offered as. Every image Look files is a PNG.
const IMAGE_PNG: &str = "image/png";

/// The verb `x-special/gnome-copied-files` opens with. Look never cuts.
const COPY_VERB: &str = "copy";

/// Bits per unit of the payload: bytes, for every type here.
const BYTE_FORMAT: i32 = 8;

/// How long a caller off the main thread waits for the grab's answer before
/// treating it as failed and shelling out.
const GRAB_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// One form the copy is offered in: every MIME spelling that asks for it, and
/// the bytes whoever asks receives.
struct Form {
    targets: &'static [&'static str],
    payload: Vec<u8>,
}

impl Form {
    fn new(targets: &'static [&'static str], payload: impl Into<Vec<u8>>) -> Self {
        Self {
            targets,
            payload: payload.into(),
        }
    }
}

pub(crate) fn copy_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    let (gnome, uri_list, text) = payloads(paths);
    let forms = vec![
        Form::new(&[GNOME_COPIED_FILES], gnome.clone()),
        Form::new(&[URI_LIST], uri_list),
        Form::new(&TEXT_TARGETS, text),
    ];
    if own_clipboard(forms) {
        return Ok(());
    }
    // No display to grab, or the selection went to someone else, so fall back to
    // the one type a file manager needs most.
    shell_out(GNOME_COPIED_FILES, gnome.as_bytes())
}

/// Puts a copied image back on the clipboard the way macOS does: the pixels
/// for an editor, the file for a file manager, the path for a text field.
/// `true` when the grab took, so the copy carries the path as text too and the
/// monitor sees a text event; `false` when only the pixels went out.
pub(crate) fn copy_image(path: &std::path::Path) -> Result<bool, String> {
    let png = std::fs::read(path).map_err(|e| format!("Failed to read the image: {e}"))?;
    let native = path.to_string_lossy().into_owned();
    let (gnome, uri_list, text) = payloads(std::slice::from_ref(&native));

    let forms = vec![
        Form::new(&[IMAGE_PNG], png.clone()),
        Form::new(&[GNOME_COPIED_FILES], gnome),
        Form::new(&[URI_LIST], uri_list),
        Form::new(&TEXT_TARGETS, text),
    ];
    if own_clipboard(forms) {
        return Ok(true);
    }
    // The pixels are what the copy was for, so that is the form worth saving.
    shell_out(IMAGE_PNG, &png).map(|_| false)
}

/// The three forms one file copy is offered in: [`GNOME_COPIED_FILES`],
/// [`URI_LIST`], and the plain text a text field pastes.
fn payloads(paths: &[String]) -> (String, String, String) {
    let uris: Vec<String> = paths.iter().map(|path| super::file_uri(path)).collect();
    (
        format!("{COPY_VERB}\n{}", uris.join("\n")),
        // text/uri-list is CRLF-delimited, per RFC 2483.
        uris.join("\r\n"),
        paths.join("\n"),
    )
}

/// Whether the grab took, so a failed one still reaches the shell fallback.
///
/// Every GTK call belongs to the main thread. A sync Tauri command already
/// answers there, so the usual path runs [`grab`] outright; a caller from
/// anywhere else queues it and waits for the answer.
fn own_clipboard(forms: Vec<Form>) -> bool {
    if gtk::is_initialized_main_thread() {
        return grab(forms);
    }

    let Some(app) = crate::state::app_handle() else {
        return false;
    };
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
fn grab(forms: Vec<Form>) -> bool {
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

/// wl-copy (Wayland) then xclip (X11), neither a hard runtime dependency. One
/// invocation advertises one MIME type, which is why the GTK path above
/// exists, so the caller picks the form worth keeping.
fn shell_out(mime: &str, payload: &[u8]) -> Result<(), String> {
    let attempts: [(&str, &[&str]); 2] = [
        ("wl-copy", &["-t", mime]),
        ("xclip", &["-selection", "clipboard", "-t", mime]),
    ];

    let mut last = String::new();
    for (program, args) in attempts {
        let outcome = super::host_command(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut child| {
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(payload)?;
                }
                child.wait()
            });

        match outcome {
            Ok(status) if status.success() => return Ok(()),
            // wl-copy on an X11 session finds no display and exits non-zero,
            // which is exactly when xclip is the one that can do it.
            Ok(status) => last = format!("{program} exited with {status}"),
            Err(e) => last = format!("{program}: {e}"),
        }
    }

    Err(format!(
        "Failed to copy: {last}. Install xclip or wl-clipboard."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each form has its own delimiter and its own idea of what a path is, and
    /// a pasting app reads whichever one it asked for verbatim.
    #[test]
    fn each_form_is_written_the_way_its_asker_reads_it() {
        let paths = vec!["/tmp/a b.txt".to_string(), "/tmp/c.txt".to_string()];
        let (gnome, uri_list, text) = payloads(&paths);

        assert_eq!(gnome, "copy\nfile:///tmp/a%20b.txt\nfile:///tmp/c.txt");
        assert_eq!(uri_list, "file:///tmp/a%20b.txt\r\nfile:///tmp/c.txt");
        assert_eq!(text, "/tmp/a b.txt\n/tmp/c.txt");
    }

    /// One path is the common case, and it must not trail a delimiter.
    #[test]
    fn a_single_path_carries_no_separator() {
        let (gnome, uri_list, text) = payloads(&["/tmp/a.txt".to_string()]);

        assert_eq!(gnome, "copy\nfile:///tmp/a.txt");
        assert_eq!(uri_list, "file:///tmp/a.txt");
        assert_eq!(text, "/tmp/a.txt");
    }
}
