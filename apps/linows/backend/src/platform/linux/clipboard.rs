//! Linux file clipboard: one copy that a file manager pastes as a file and a
//! text field pastes as a path.
//!
//! A clipboard advertises a *list* of types and the pasting app asks for the
//! one it understands, which is how macOS writes a URL and a string in a single
//! copy (`NSPasteboard.writeObjects([url, path])`). `wl-copy` and `xclip` each
//! advertise one type per invocation, so Look owns the clipboard itself through
//! the shell's toolkit and answers whichever type is requested. They stay as
//! the fallback for the case where the grab fails.

use std::io::Write;
use std::process::Stdio;

use crate::host::ClipForm as Form;

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

fn form(targets: &'static [&'static str], payload: impl Into<Vec<u8>>) -> Form {
    Form {
        targets,
        payload: payload.into(),
    }
}

pub(crate) fn copy_files(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    let (gnome, uri_list, text) = payloads(paths);
    let forms = vec![
        form(&[GNOME_COPIED_FILES], gnome.clone()),
        form(&[URI_LIST], uri_list),
        form(&TEXT_TARGETS, text),
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
        form(&[IMAGE_PNG], png.clone()),
        form(&[GNOME_COPIED_FILES], gnome),
        form(&[URI_LIST], uri_list),
        form(&TEXT_TARGETS, text),
    ];
    if own_clipboard(forms) {
        return Ok(true);
    }
    // The pixels are what the copy was for, so that is the form worth saving.
    shell_out(IMAGE_PNG, &png).map(|_| false)
}

/// Text, in every spelling a pasting app might ask for. Through the shell like
/// the rest, so a copy needs no X server on a Wayland session.
pub(crate) fn copy_text(text: &str) -> Result<(), String> {
    if own_clipboard(vec![form(&TEXT_TARGETS, text.as_bytes())]) {
        return Ok(());
    }
    shell_out(TEXT_TARGETS[0], text.as_bytes())
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
fn own_clipboard(forms: Vec<Form>) -> bool {
    crate::host::host().is_some_and(|host| host.own_clipboard(forms))
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
