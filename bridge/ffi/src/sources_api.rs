//! C-ABI wrappers over `look_engine::sources`, so the Swift shell can read a
//! user-declared block, list what a row can go on to, and perform it.
//!
//! Transport only. Resolving a candidate id back to its block, expanding a
//! target's `confirm` against the row, the depth cap and the drilled id
//! encoding all live in the engine, shared with the Tauri shell, so the two
//! cannot answer the same question differently.

use look_sources::ParentRow;
use serde::Serialize;
use std::os::raw::c_char;

use crate::state::{cstr_to_string, json_cstring_or_null, mark_index_dirty};

/// `{id, name, steps, file, then}` for the block a candidate id belongs to, or
/// the JSON literal `null` when it is not a block row or no longer exists.
pub(crate) fn look_source_block_json_impl(
    candidate_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    ancestors_json: *const c_char,
) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    let row = row_context(row_id, row_title, row_path, "", ancestors_json);
    json(look_engine::sources::block_detail(&candidate_id, &row))
}

/// Every declared block as `{id, name, icon}`. The shell caches this once per
/// launcher open so a row can show its declared icon without a disk read each
/// time it renders.
pub(crate) fn look_source_blocks_json_impl() -> *mut c_char {
    json(Some(look_engine::sources::block_summaries()))
}

/// Performs `block_id` against the selected row, which is what its placeholders
/// expand to. Returns `{performed, errors, produces_rows, opens_path}`.
pub(crate) fn look_perform_block_json_impl(
    block_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    query: *const c_char,
    ancestors_json: *const c_char,
    as_target: bool,
) -> *mut c_char {
    let block_id = cstr_to_string(block_id);
    let row = row_context(
        row_id,
        row_title,
        row_path,
        &cstr_to_string(query),
        ancestors_json,
    );
    json(Some(look_engine::sources::perform_block(
        &block_id, &row, as_target,
    )))
}

/// Re-runs every enabled `run` block and stores its rows, so the next index pass
/// picks them up. Returns `{refreshed, errors}`.
pub(crate) fn look_refresh_run_blocks_json_impl() -> *mut c_char {
    let outcome = look_engine::sources::refresh_run_blocks();
    // Rows land in a cache directory no watcher covers, so nothing else says so.
    if outcome.changed {
        mark_index_dirty();
    }
    json(Some(outcome))
}

/// The rows of `block_id` produced against the selected row, for descending.
pub(crate) fn look_source_rows_json_impl(
    block_id: *const c_char,
    parent_candidate_id: *const c_char,
    parent_title: *const c_char,
    parent_path: *const c_char,
    query: *const c_char,
    ancestors_json: *const c_char,
) -> *mut c_char {
    json(Some(look_engine::sources::level(
        &cstr_to_string(block_id),
        &cstr_to_string(parent_candidate_id),
        &cstr_to_string(parent_title),
        &cstr_to_string(parent_path),
        &cstr_to_string(query),
        parents_from(ancestors_json),
    )))
}

/// Runs a block's declared `preview` against the selected row and returns its
/// output as `{text, error}`, or `null` when it declares none.
pub(crate) fn look_source_preview_json_impl(
    candidate_id: *const c_char,
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    ancestors_json: *const c_char,
) -> *mut c_char {
    let candidate_id = cstr_to_string(candidate_id);
    let row = row_context(row_id, row_title, row_path, "", ancestors_json);
    json(look_engine::sources::preview(&candidate_id, &row))
}

/// The selected row as a user's command sees it.
fn row_context(
    row_id: *const c_char,
    row_title: *const c_char,
    row_path: *const c_char,
    query: &str,
    ancestors_json: *const c_char,
) -> look_sources::RowContext {
    look_engine::sources::row_context(
        &cstr_to_string(row_id),
        &cstr_to_string(row_title),
        &cstr_to_string(row_path),
        query,
        parents_from(ancestors_json),
    )
}

/// The rows a drilled row was reached through, for `{parent.*}`.
pub(crate) fn parents_from(ancestors_json: *const c_char) -> Vec<ParentRow> {
    look_engine::sources::parents_from_json(&cstr_to_string(ancestors_json))
}

pub(crate) fn find_block(block_id: &str) -> Option<look_sources::Block> {
    look_engine::sources::find_block(block_id)
}

/// `None` becomes the JSON literal `null`, which every caller already reads as
/// "nothing to show".
fn json<T: Serialize>(value: Option<T>) -> *mut c_char {
    json_cstring_or_null(value.and_then(|value| serde_json::to_string(&value).ok()))
}

/// The config file this build reads and writes, migrating a legacy
/// `~/.look.config` into `~/.look/` the first time.
///
/// The shell asks Rust rather than deciding for itself, so the two can never
/// resolve differently and end up reading one file while saving to another.
pub(crate) fn look_config_path_impl(dev: bool) -> *mut c_char {
    let Some(home) = home_dir() else {
        return json_cstring_or_null(None);
    };
    let resolved = look_engine::config_path::resolve_home_variant(&home, dev);
    json_cstring_or_null(Some(resolved.path.to_string_lossy().into_owned()))
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use look_tools::cmd_quote as quote;
    #[cfg(not(windows))]
    use look_tools::shell_quote as quote;
    use std::ffi::{CStr, CString};
    use std::path::PathBuf;

    /// The sources directory is process-wide state, so these run one at a time.
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new(label: &str, declarations: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("look-ffi-src-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("fixture dir");
            std::fs::write(dir.join("blocks.toml"), declarations).expect("fixture file");
            unsafe { std::env::set_var(look_sources::SOURCES_DIR_ENV, &dir) };
            Self { dir }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(look_sources::SOURCES_DIR_ENV) };
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn tool_action(
        action: &str,
        candidate: &str,
        title: &str,
        path: &str,
        ancestors: &str,
    ) -> serde_json::Value {
        let action = CString::new(action).unwrap();
        let candidate = CString::new(candidate).unwrap();
        let title = CString::new(title).unwrap();
        let path = CString::new(path).unwrap();
        let ancestors = CString::new(ancestors).unwrap();
        let ptr = crate::tools_api::look_tool_action_json_impl(
            action.as_ptr(),
            candidate.as_ptr(),
            title.as_ptr(),
            path.as_ptr(),
            true,
            ancestors.as_ptr(),
        );
        let raw = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        crate::state::free_json_allocation(ptr);
        serde_json::from_str(&raw).expect("resolved action json")
    }

    #[test]
    fn a_blocks_verb_takes_the_chord_for_its_own_rows_only() {
        let _guard = GUARD.lock().unwrap_or_else(|err| err.into_inner());
        let _fixture = Fixture::new(
            "chords",
            "[projects]\ndir = \"/tmp\"\nterminal = \"tmux new -As {title} -c {parent.path}\"\n",
        );

        // The block's own row: its verb wins, expanded like any other command
        // it declares, ancestors included.
        let mine = tool_action(
            "terminal",
            "src:projects:/tmp/look",
            "look",
            "/tmp/look",
            r#"[{"id":"dev","title":"dev","path":"/dev"}]"#,
        );
        assert_eq!(mine["kind"], "shell", "{mine}");
        // The platform's quoting, so this says which placeholder was quoted
        // rather than repeating one shell's spelling of it.
        assert_eq!(
            mine["command"],
            format!("tmux new -As {} -c {}", quote("look"), quote("/dev"))
        );
        assert_eq!(mine["tool"], "projects", "the block is what decided");

        // A chord it did not declare is untouched, so a block cannot quietly
        // take over keys it never mentioned.
        assert_ne!(
            tool_action("edit", "src:projects:/tmp/look", "look", "/tmp/look", "[]")["tool"],
            serde_json::json!("projects")
        );

        // An ordinary file row never sees the block at all.
        let theirs = tool_action("terminal", "file:/tmp/other", "other", "/tmp/other", "[]");
        assert_ne!(theirs["tool"], serde_json::json!("projects"), "{theirs}");
    }
}
