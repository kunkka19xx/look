//! C-ABI wrappers over `look_engine::lookbook`, so the shell can show what a
//! lookbook entry would do here and then install it.
//!
//! Transport only. The shell fetches the text, because `URLSession` is where
//! this app's network already lives; everything said about that text is
//! decided in the engine, shared with the Tauri shell.

use std::os::raw::c_char;

use serde::Serialize;

use crate::state::{cstr_to_string, home_dir, json_cstring_or_null, mark_index_dirty};

/// What `toml` declares and what it would need on this machine, as
/// `{blocks, problems}`, or the JSON literal `null` when it is not a file of
/// blocks at all.
pub(crate) fn look_lookbook_preview_json(toml: *const c_char) -> *mut c_char {
    let toml = cstr_to_string(toml);
    json(home_dir().and_then(|home| look_engine::lookbook::preview(&toml, &home).ok()))
}

/// Which of `programs_json` (a JSON array of names) are not installed here.
/// For a catalog entry's `requires`, which names a program with no block
/// behind it.
pub(crate) fn look_lookbook_missing_json(programs_json: *const c_char) -> *mut c_char {
    // Null on input that is not a list of names, rather than an empty list:
    // `[]` already means "nothing missing", and the two calls either side of
    // this one answer null when they cannot read what they were given.
    let programs: Option<Vec<String>> = serde_json::from_str(&cstr_to_string(programs_json)).ok();
    json(programs.map(|programs| look_engine::lookbook::missing_programs(&programs)))
}

/// Which of an entry's files are already on disk, as
/// `{declarations, scripts}`, so the sheet can say what it would replace
/// before it writes. Takes `{declarations: [name], scripts: [name]}`.
pub(crate) fn look_lookbook_present_json(request_json: *const c_char) -> *mut c_char {
    let Ok(request) = serde_json::from_str(&cstr_to_string(request_json)) else {
        return json_cstring_or_null(None);
    };
    json(home_dir().map(|home| look_engine::lookbook::present(&request, &home)))
}

/// Writes an entry into the user's directories. Takes
/// `{declarations: [{name, contents}], scripts: [...]}` and returns
/// `{written, replaced, problems}`.
pub(crate) fn look_lookbook_install_json(request_json: *const c_char) -> *mut c_char {
    let Ok(request) = serde_json::from_str(&cstr_to_string(request_json)) else {
        return json_cstring_or_null(None);
    };
    let Some(home) = home_dir() else {
        return json_cstring_or_null(None);
    };
    let outcome = look_engine::lookbook::install(&request, &home);
    // New rows exist the moment the file lands, and the index does not watch
    // this directory. Without this the user installs something and sees
    // nothing until the next reload.
    if !outcome.written.is_empty() {
        mark_index_dirty();
    }
    json(Some(outcome))
}

fn json<T: Serialize>(value: Option<T>) -> *mut c_char {
    json_cstring_or_null(value.and_then(|value| serde_json::to_string(&value).ok()))
}
