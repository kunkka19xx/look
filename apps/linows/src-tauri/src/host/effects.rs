//! Windows-specific window effects (Mica, Acrylic).
//!
//! Rounded corners are NOT here: DWM's `DWMWA_WINDOW_CORNER_PREFERENCE`
//! returns S_OK but is a verified no-op on `transparent: true` windows
//! (per MS's "Apply rounded corners" doc - per-pixel-alpha layered windows
//! are excluded). Corners are achieved via CSS `border-radius` on
//! `.launcher-window` scoped under `[data-os="windows"]` in `layout.css`.

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
