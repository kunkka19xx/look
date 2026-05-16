//! Windows-specific window effects (Mica, Acrylic).

use tauri::utils::config::WindowEffectsConfig;
use tauri::window::Effect;

pub(crate) fn apply(window: tauri::Window, effect: &str) -> Result<(), String> {
    let config: Option<WindowEffectsConfig> = match effect {
        "mica" => Some(WindowEffectsConfig {
            effects: vec![Effect::Mica],
            ..Default::default()
        }),
        "acrylic" => Some(WindowEffectsConfig {
            effects: vec![Effect::Acrylic],
            ..Default::default()
        }),
        "none" | "" => None,
        _ => return Err(format!("Unknown effect: {effect}")),
    };

    window
        .set_effects(config)
        .map_err(|e| format!("Failed to set effect: {e}"))
}
