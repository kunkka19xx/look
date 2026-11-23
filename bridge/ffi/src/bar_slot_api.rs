//! The search bar slot for the macOS shell, over `look_engine::bar_slot`.

use crate::state::{cstr_to_string, json_cstring_or_null, with_engine};
use look_engine::bar_slot::{self, BarSlot};
use serde::Serialize;
use std::collections::HashMap;
use std::os::raw::c_char;

#[derive(Serialize)]
struct BarConfig {
    slot: &'static str,
    pinned: Vec<String>,
    script: String,
}

#[derive(Serialize)]
struct Pin<'a> {
    id: String,
    title: Option<&'a str>,
    path: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum ScriptOutcome {
    Line(String),
    Error(String),
}

fn bar_config(entries: &HashMap<String, String>) -> BarConfig {
    let value = |key: &str| entries.get(key).map(|v| v.trim()).unwrap_or_default();
    BarConfig {
        slot: BarSlot::from_entries(entries).as_str(),
        pinned: bar_slot::pinned_ids(value(bar_slot::PINNED_APPS_KEY)),
        script: value(bar_slot::SCRIPT_KEY).to_string(),
    }
}

pub(crate) fn look_bar_config_json_impl(entries_json: *const c_char) -> *mut c_char {
    let entries: HashMap<String, String> =
        serde_json::from_str(&cstr_to_string(entries_json)).unwrap_or_default();
    json_cstring_or_null(serde_json::to_string(&bar_config(&entries)).ok())
}

pub(crate) fn look_bar_pins_json_impl(ids_json: *const c_char) -> *mut c_char {
    let ids: Vec<String> = serde_json::from_str(&cstr_to_string(ids_json)).unwrap_or_default();
    let json = with_engine(|engine| {
        let pins: Vec<Pin> = ids
            .into_iter()
            .map(|id| {
                let (title, path) = engine.title_and_path(&id).unzip();
                Pin { id, title, path }
            })
            .collect();
        serde_json::to_string(&pins).ok()
    });
    json_cstring_or_null(json)
}

pub(crate) fn look_bar_script_line_json_impl(command: *const c_char) -> *mut c_char {
    let outcome = match bar_slot::script_line(&cstr_to_string(command)) {
        Ok(line) => ScriptOutcome::Line(line),
        Err(why) => ScriptOutcome::Error(why),
    };
    json_cstring_or_null(serde_json::to_string(&outcome).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn the_config_carries_the_slot_pins_and_script() {
        let config = bar_config(&entries(&[
            (bar_slot::SLOT_KEY, "pinned"),
            (bar_slot::PINNED_APPS_KEY, "app:a, app:b\\,c"),
            (bar_slot::SCRIPT_KEY, "  date  "),
        ]));
        assert_eq!(config.slot, "pinned");
        assert_eq!(config.pinned, vec!["app:a", "app:b,c"]);
        assert_eq!(config.script, "date");
    }

    #[test]
    fn an_old_config_keeps_its_bar() {
        let config = bar_config(&entries(&[(bar_slot::LEGACY_PLACEMENT_KEY, "none")]));
        assert_eq!(config.slot, "none");
        assert!(config.pinned.is_empty());
        assert!(config.script.is_empty());
    }

    #[test]
    fn a_script_outcome_is_tagged_by_its_kind() {
        let line = serde_json::to_string(&ScriptOutcome::Line("09:41".into())).unwrap();
        let error = serde_json::to_string(&ScriptOutcome::Error("boom".into())).unwrap();
        assert_eq!(line, r#"{"line":"09:41"}"#);
        assert_eq!(error, r#"{"error":"boom"}"#);
    }
}
