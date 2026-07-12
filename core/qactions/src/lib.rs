//! Quick Actions - shared action catalog
//!
//! This crate is the platform-neutral half of the framework: it declares WHICH
//! actions exist and their presentation (labels, control kind, info fields). It
//! knows nothing about how to execute them - that is a native `SystemControl`
//! adapter, keyed by `action_id`, in each platform shell.
//!
//! A contributor adds a control by (1) declaring its descriptor here, (2) binding
//! the platform result id(s) that trigger it, and (3) implementing the native
//! adapter. Only steps 1-2 live in this crate.

use serde::Serialize;

/// How an action's control renders in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlKind {
    /// A boolean on/off switch.
    Toggle,
    /// A plain trigger button.
    Button,
}

/// A read-only field shown above the actions. The core declares the label and a
/// `value_key`; the native adapter resolves the key to a live value for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfoFieldSpec {
    pub label: String,
    pub value_key: String,
}

/// A declared action. Serialized to the platform shells; `action_id` selects the
/// native adapter that runs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionDescriptor {
    pub action_id: String,
    pub title: String,
    pub control: ControlKind,
    pub on_label: Option<String>,
    pub off_label: Option<String>,
    pub info: Vec<InfoFieldSpec>,
}

/// The action definition for `action_id`, or `None` if unknown. This is the
/// shared, platform-neutral catalog - add new controls here.
pub fn descriptor(action_id: &str) -> Option<ActionDescriptor> {
    match action_id {
        "bluetooth" => Some(ActionDescriptor {
            action_id: "bluetooth".to_string(),
            title: "Bluetooth".to_string(),
            control: ControlKind::Toggle,
            on_label: Some("On".to_string()),
            off_label: Some("Off".to_string()),
            info: vec![InfoFieldSpec {
                label: "Status".to_string(),
                value_key: "status".to_string(),
            }],
        }),
        _ => None,
    }
}

/// Descriptors that apply to a search result. Resolves the (platform-specific)
/// result id to a shared `action_id` via [`binding_for`], then looks up the
/// definition. Empty when the result has no actions.
pub fn descriptors_for(result_id: &str, kind: &str) -> Vec<ActionDescriptor> {
    binding_for(result_id, kind)
        .and_then(descriptor)
        .into_iter()
        .collect()
}

/// Maps a platform result to a shared `action_id`. The action DEFINITIONS above
/// are shared; only which platform result triggers them is per-OS, so this is
/// `cfg`-gated. A contributor adding an OS binds its result id here.
#[cfg(target_os = "macos")]
fn binding_for(result_id: &str, _kind: &str) -> Option<&'static str> {
    match result_id {
        "setting:com.apple.bluetoothsettings" => Some("bluetooth"),
        _ => None,
    }
}

#[cfg(not(target_os = "macos"))]
fn binding_for(_result_id: &str, _kind: &str) -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bluetooth_descriptor_is_a_toggle_with_labels() {
        let d = descriptor("bluetooth").expect("bluetooth defined");
        assert_eq!(d.control, ControlKind::Toggle);
        assert_eq!(d.on_label.as_deref(), Some("On"));
        assert_eq!(d.off_label.as_deref(), Some("Off"));
        assert_eq!(d.info.len(), 1);
    }

    #[test]
    fn unknown_action_is_none() {
        assert!(descriptor("nope").is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bluetooth_setting_resolves_to_the_toggle() {
        let found = descriptors_for("setting:com.apple.bluetoothsettings", "app");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].action_id, "bluetooth");
        // A non-actionable result yields nothing.
        assert!(descriptors_for("setting:com.apple.wifi-settings-extension", "app").is_empty());
    }
}
