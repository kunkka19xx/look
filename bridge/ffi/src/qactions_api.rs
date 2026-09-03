//! C-ABI wrapper over `look_qactions`, so the platform shells can fetch the
//! Quick Action descriptors for a selected result. Read the result id + kind,
//! call the shared catalog, hand back a JSON array (empty `[]` on no match or
//! any failure). Mirrors `answers_api`.

use crate::state::{cstr_to_string, store_json_allocation};
use std::ffi::CString;
use std::os::raw::c_char;

const JSON_EMPTY_ARRAY: &str = "[]";

/// Hands `json` to the shells, falling back to `empty` if it holds a NUL.
fn store_json(json: String, empty: &str) -> *mut c_char {
    let cstring = CString::new(json).unwrap_or_else(|_| CString::new(empty).expect("valid"));
    store_json_allocation(cstring)
}

/// JSON array of `ActionDescriptor` for the result `(result_id, kind)`, or `[]`.
pub(crate) fn look_qactions_json_impl(
    result_id: *const c_char,
    kind: *const c_char,
) -> *mut c_char {
    let result_id = cstr_to_string(result_id);
    let kind = cstr_to_string(kind);
    let descriptors = look_qactions::descriptors_for(&result_id, &kind);
    let json = serde_json::to_string(&descriptors).unwrap_or_else(|_| JSON_EMPTY_ARRAY.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_EMPTY_ARRAY).expect("valid"));
    store_json_allocation(cstring)
}

/// The empty-state launchpad layout as `{columns, rows, tiles}`, every tile
/// carrying the cell it was resolved to, or `[]` on a serialization failure.
///
/// Reads `~/.look/super-actions.toml` through the engine, which falls back to the
/// built-in grid when there is no file or it cannot be trusted - so this never
/// answers with an empty layout for a reason the user could have fixed. Still
/// takes no arguments: the drawing is the only input and it is on disk.
pub(crate) fn look_quick_actions_launchpad_json_impl() -> *mut c_char {
    let payload = look_engine::launchpad::layout_payload();
    let json = serde_json::to_string(&payload).unwrap_or_else(|_| JSON_EMPTY_ARRAY.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_EMPTY_ARRAY).expect("valid"));
    store_json_allocation(cstring)
}

/// Anything wrong with `~/.look/super-actions.toml`, as a JSON array of strings, or
/// `[]` when it is fine, absent, or unreadable in a way already handled.
///
/// Its own call rather than a field beside the tiles: the layout is re-read on
/// every reload, and these are wanted only when there is somewhere to show them.
///
/// Resolves silently - the layout call has already printed these to stderr, and
/// this exists to put them somewhere a user who did not launch from a terminal
/// can see.
pub(crate) fn look_launchpad_warnings_json_impl() -> *mut c_char {
    let warnings = look_engine::launchpad::layout().warnings;
    let json = serde_json::to_string(&warnings).unwrap_or_else(|_| JSON_EMPTY_ARRAY.to_string());
    let cstring =
        CString::new(json).unwrap_or_else(|_| CString::new(JSON_EMPTY_ARRAY).expect("valid"));
    store_json_allocation(cstring)
}

/// What each user tile currently shows. Reads the cache; runs nothing, so it is
/// safe on the path that draws the strip.
pub(crate) fn look_launchpad_tile_values_json_impl() -> *mut c_char {
    let values = look_engine::launchpad_values::cached();
    store_json(
        serde_json::to_string(&values).unwrap_or_else(|_| "{}".to_string()),
        "{}",
    )
}

/// Runs every stale tile and stores what it prints. Spawns and blocks: never on
/// the thread the launchpad is drawn on.
pub(crate) fn look_launchpad_refresh_tiles_json_impl() -> *mut c_char {
    let outcome = look_engine::launchpad_values::refresh();
    let payload = serde_json::json!({
        "refreshed": outcome.refreshed,
        "errors": outcome.errors,
    });
    store_json(payload.to_string(), "{}")
}

/// Runs a user tile's `press`, named by the tile. The command never crosses this
/// boundary, so a shell cannot run something the drawing did not declare.
pub(crate) fn look_launchpad_press_tile_json_impl(name: *const c_char) -> *mut c_char {
    let name = cstr_to_string(name);
    let error = look_engine::launchpad_values::press(&name).err();
    store_json(serde_json::json!({ "error": error }).to_string(), "{}")
}

#[cfg(test)]
mod tests {
    /// The exact JSON both shells decode, as a file rather than an inline
    /// literal because the Swift and JS tests read this same one. A field
    /// renamed here and nowhere else fails in all three places at once, which
    /// is the only way this contract announces itself: both shells swallow a
    /// mismatch. Swift decodes with `try?` and falls back to `[]`, and JS reads
    /// keys straight off the object, so a rename renders an EMPTY launchpad
    /// rather than an error.
    const FIXTURE_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/launchpad_layout.json"
    );

    /// Every key a shell dereferences, spelled as the wire spells it.
    ///
    /// Swift decodes with `.convertFromSnakeCase`, so it reads `actionId` and
    /// `onLabel`; the JS shell reads the raw object, so it reads `action_id`
    /// and `on_label`. The two shells never read the same string, which is why
    /// pinning the wire spelling is what protects both.
    const WIRE_KEYS: [&str; 14] = [
        "action_id",
        "title",
        "size",
        "role",
        "mnemonic",
        "col",
        "row",
        "col_span",
        "row_span",
        "on_label",
        "off_label",
        "pressable",
        "has_value",
        "confirm",
    ];

    /// The payload as the bridges send it, resolved from the seeded drawing
    /// rather than from `layout_payload`: that reads the developer's own
    /// ~/.look/super-actions.toml, which would fail this test on any customised
    /// machine and write that private layout into the shared fixture.
    fn live_payload() -> serde_json::Value {
        let resolved = look_engine::launchpad::resolve(look_engine::launchpad::default_contents());
        let payload = look_engine::launchpad::LayoutPayload::from(resolved);
        serde_json::to_value(payload).expect("the layout serialises")
    }

    /// Its own fixture because the default has no user tile, and that gap hid a
    /// real break: an unknown `role` failed the whole decode on macOS and
    /// `try?` swallowed it, emptying the launchpad silently.
    const CUSTOM_FIXTURE_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/launchpad_custom_tile.json"
    );

    fn custom_payload() -> serde_json::Value {
        let resolved = look_engine::launchpad::resolve(
            r#"
layout = ["ci  ci"]

[tiles.ci]
value    = "~/.look/bin/ci"
press    = "open https://ci.example.com"
mnemonic = "C"
"#,
        );
        let payload = look_engine::launchpad::LayoutPayload::from(resolved);
        serde_json::to_value(payload).expect("the layout serialises")
    }

    /// Compares against the checked-in fixture, or rewrites it under
    /// UPDATE_FIXTURES. Both shells' tests read these same files, so a wire
    /// change fails in all three places at once.
    fn assert_matches_fixture(live: &serde_json::Value, path: &str) {
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            let pretty = serde_json::to_string_pretty(live).expect("serialises");
            std::fs::write(path, format!("{pretty}\n")).expect("fixture is writable");
            return;
        }
        let fixture: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("fixture is there"))
                .expect("the fixture is valid JSON");
        assert_eq!(
            live, &fixture,
            "the launchpad wire format changed. Regenerate with UPDATE_FIXTURES=1 \
             cargo test --manifest-path bridge/ffi/Cargo.toml"
        );
    }

    #[test]
    fn a_user_tile_survives_the_round_trip_to_both_shells() {
        let live = custom_payload();
        // The tile really is the user's, not a built-in that happens to parse.
        assert_eq!(live["tiles"][0]["role"], "custom");
        assert_matches_fixture(&live, CUSTOM_FIXTURE_PATH);
    }

    #[test]
    fn the_launchpad_layout_survives_the_round_trip_to_both_shells() {
        assert_matches_fixture(&live_payload(), FIXTURE_PATH);
    }

    #[test]
    fn a_layout_that_decodes_to_nothing_is_indistinguishable_from_a_broken_one() {
        let live = live_payload();
        let payload = live.as_object().expect("a payload object");
        for key in ["columns", "rows", "tiles"] {
            assert!(
                payload.contains_key(key),
                "the payload is missing {key}, which both shells read by that name"
            );
        }
        let tiles = payload["tiles"].as_array().expect("an array of tiles");

        // Non-empty is the assertion that matters: both shells fall back to an
        // empty list on a decode failure, so "no tiles" is exactly what a
        // broken contract looks like from the outside.
        assert!(!tiles.is_empty(), "an empty layout is the failure mode");

        for tile in tiles {
            let tile = tile.as_object().expect("a tile is an object");
            for key in WIRE_KEYS {
                assert!(
                    tile.contains_key(key),
                    "tile {:?} is missing {key}, which a shell reads by that exact name",
                    tile.get("action_id")
                );
            }
        }
    }
}
