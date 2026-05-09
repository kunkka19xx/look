use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ConfigPayload {
    pub path: String,
    pub entries: Vec<ConfigEntry>,
}

#[derive(Serialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
pub struct ConfigUpdate {
    pub key: String,
    pub value: String,
}

#[tauri::command]
pub fn get_config() -> ConfigPayload {
    let path = config_file_path();
    let path_str = path.to_string_lossy().to_string();
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let mut entries = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            entries.push(ConfigEntry {
                key: key.trim().to_string(),
                value: value.trim().to_string(),
            });
        }
    }
    ConfigPayload {
        path: path_str,
        entries,
    }
}

#[tauri::command]
pub fn set_config(updates: Vec<ConfigUpdate>) -> Result<(), String> {
    let path = config_file_path();
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = contents.lines().map(|l| l.to_string()).collect();

    for update in &updates {
        let new_line = format!("{}={}", update.key, update.value);
        let mut found = false;
        for line in &mut lines {
            let trimmed = line.trim();
            if !trimmed.starts_with('#')
                && let Some((k, _)) = trimmed.split_once('=')
                && k.trim() == update.key
            {
                *line = new_line.clone();
                found = true;
                break;
            }
        }
        if !found {
            lines.push(new_line);
        }
    }

    let mut output = lines.join("\n");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    std::fs::write(&path, output).map_err(|e| format!("Failed to write config: {e}"))
}

#[tauri::command]
pub fn reset_config() -> Result<(), String> {
    let path = config_file_path();
    let default = include_str!("default_config.txt");
    std::fs::write(&path, default).map_err(|e| format!("Failed to reset config: {e}"))
}

pub fn config_file_path() -> std::path::PathBuf {
    if let Ok(custom) = std::env::var("LOOK_CONFIG_PATH") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return std::path::PathBuf::from(trimmed);
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".look.config")
}
