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

/// Which payload a request wants. A target reaches the getter as a number, so
/// these are what the callback switches on.
const INFO_GNOME: u32 = 0;
const INFO_URI_LIST: u32 = 1;
const INFO_TEXT: u32 = 2;

/// The verb `x-special/gnome-copied-files` opens with. Look never cuts.
const COPY_VERB: &str = "copy";

/// Bits per unit of the payload: bytes, for every type here.
const BYTE_FORMAT: i32 = 8;

pub(crate) fn copy_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    let uris: Vec<String> = paths.iter().map(|path| file_uri(path)).collect();
    let gnome = format!("{COPY_VERB}\n{}", uris.join("\n"));

    if own_clipboard(gnome.clone(), uris.join("\r\n"), paths.join("\n")) {
        return Ok(());
    }
    // No display connection to grab (headless, or GTK not up yet), so fall back
    // to the one type a file manager needs most.
    shell_out(&gnome)
}

/// Hand the payloads to GTK and become the clipboard owner.
///
/// The work is queued onto the main thread rather than run here: every GTK call
/// belongs to it, and a Tauri command answers on a worker. Returns false when
/// that thread cannot be reached at all.
fn own_clipboard(gnome: String, uri_list: String, text: String) -> bool {
    let Some(app) = crate::state::app_handle() else {
        return false;
    };

    app.run_on_main_thread(move || {
        let Some(display) = gdk::Display::default() else {
            return;
        };
        let clipboard = gtk::Clipboard::for_display(&display, &gdk::SELECTION_CLIPBOARD);

        let mut targets = vec![
            TargetEntry::new(GNOME_COPIED_FILES, TargetFlags::empty(), INFO_GNOME),
            TargetEntry::new(URI_LIST, TargetFlags::empty(), INFO_URI_LIST),
        ];
        targets.extend(
            TEXT_TARGETS
                .iter()
                .map(|target| TargetEntry::new(target, TargetFlags::empty(), INFO_TEXT)),
        );

        // Answered on demand, once per paste, for as long as Look holds the
        // clipboard, which is why the payloads are moved in rather than
        // borrowed.
        let owned = clipboard.set_with_data(&targets, move |_, selection, info| {
            let payload = match info {
                INFO_GNOME => &gnome,
                INFO_URI_LIST => &uri_list,
                _ => &text,
            };
            selection.set(&selection.target(), BYTE_FORMAT, payload.as_bytes());
        });

        if owned {
            allow_manager_to_store(&clipboard);
        }
    })
    .is_ok()
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

/// `file://` with every byte outside the unreserved set percent-encoded.
fn file_uri(path: &str) -> String {
    let mut encoded = String::from("file://");
    for byte in path.as_bytes() {
        match byte {
            b'/' | b'-' | b'.' | b'_' | b'~' => encoded.push(*byte as char),
            _ if byte.is_ascii_alphanumeric() => encoded.push(*byte as char),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// wl-copy (Wayland) then xclip (X11), neither a hard runtime dependency. Only
/// the file-manager type: one invocation advertises one MIME type, which is the
/// whole reason the GTK path above exists.
fn shell_out(payload: &str) -> Result<(), String> {
    let attempts: [(&str, &[&str]); 2] = [
        ("wl-copy", &["-t", GNOME_COPIED_FILES]),
        (
            "xclip",
            &["-selection", "clipboard", "-t", GNOME_COPIED_FILES],
        ),
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
                    stdin.write_all(payload.as_bytes())?;
                }
                child.wait()
            });

        match outcome {
            Ok(_) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
    }

    Err(format!(
        "Failed to copy files: {last}. Install xclip or wl-clipboard."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path with a space is what breaks a naive `file://` + path.
    #[test]
    fn a_uri_encodes_everything_outside_the_unreserved_set() {
        assert_eq!(
            file_uri("/tmp/my project/a b.txt"),
            "file:///tmp/my%20project/a%20b.txt"
        );
        assert_eq!(file_uri("/tmp/a~b-c_d.txt"), "file:///tmp/a~b-c_d.txt");
    }

    /// Each payload has to reach the getter under its own number, or a paste
    /// gets a form its asker cannot read.
    #[test]
    fn every_target_group_is_told_apart() {
        let numbers = [INFO_GNOME, INFO_URI_LIST, INFO_TEXT];
        for (index, number) in numbers.iter().enumerate() {
            assert!(
                !numbers[..index].contains(number),
                "{number} is claimed twice"
            );
        }
    }
}
