use crate::index::SETTINGS_CANDIDATE_ID_PREFIX;
use crate::platform;
use crate::platform::SettingsCatalogEntry;
use look_indexing::{Candidate, CandidateKind};
use std::collections::HashMap;
use std::sync::mpsc;

pub fn discover_system_settings_entries(tx: mpsc::SyncSender<Candidate>) {
    // With a settings app (gnome-control-center family), emit the whole catalog.
    // e.g. gnome-control-center on GNOME, skipped on i3/sway/minimal distros.
    if platform::has_settings_app() {
        #[cfg(target_os = "macos")]
        let localized = build_localized_title_map();

        for entry in platform::settings_catalog() {
            #[cfg(target_os = "macos")]
            emit_entry_localized(&tx, entry, &localized);
            #[cfg(not(target_os = "macos"))]
            emit_entry(&tx, entry);
        }

        // Windows extras: classic .cpl / .msc / .exe applets that Settings
        // doesn't cover (env vars, Device Manager, Services, Registry, Task
        // Manager, …). Paths use the `look-cmd://` scheme so the Tauri launcher
        // knows to spawn them via Command::new rather than ShellExecute.
        #[cfg(target_os = "windows")]
        emit_windows_control_panel_entries(&tx);
    } else {
        emit_settings_fallback_entries(&tx);
    }
}

/// No settings app (KDE, sway, i3, minimal): the panel targets are
/// gnome-control-center URLs that won't open here, so we skip them - except
/// Bluetooth, whose quick action toggles and lists devices over BlueZ, which
/// is desktop-agnostic. Surface just that one when a controller is present.
#[cfg(target_os = "linux")]
fn emit_settings_fallback_entries(tx: &mpsc::SyncSender<Candidate>) {
    if platform::bluetooth_present()
        && let Some(entry) = platform::settings_catalog()
            .iter()
            .find(|e| e.target == "bluetooth")
    {
        emit_entry(tx, entry);
    }
}

/// Only Linux has a fallback: elsewhere a missing settings app means there is
/// nothing to surface. Kept as a real function so the branch above reads the same
/// on every platform, rather than an early return that dangles once the Linux-only
/// tail is compiled out.
#[cfg(not(target_os = "linux"))]
fn emit_settings_fallback_entries(_tx: &mpsc::SyncSender<Candidate>) {}

#[cfg(not(target_os = "macos"))]
fn emit_entry(tx: &mpsc::SyncSender<Candidate>, entry: &SettingsCatalogEntry) {
    let mut candidate = Candidate::new(
        &candidate_id(entry),
        CandidateKind::App,
        entry.title,
        &target_path(entry),
    );
    candidate.subtitle = Some(subtitle(entry).into());
    let _ = tx.send(candidate);
}

#[cfg(target_os = "macos")]
fn build_localized_title_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(entries) = std::fs::read_dir(platform::SETTINGS_EXTENSIONS_DIR) else {
        return map;
    };

    let catalog_targets: HashMap<&str, &SettingsCatalogEntry> = platform::settings_catalog()
        .iter()
        .map(|e| (e.target, e))
        .collect();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("appex") {
            continue;
        }
        let Some(path_str) = path.to_str() else {
            continue;
        };

        let plist_path = format!("{path_str}/Contents/Info.plist");
        let Ok(value) = plist::Value::from_file(&plist_path) else {
            continue;
        };
        let Some(dict) = value.as_dictionary() else {
            continue;
        };
        let Some(bundle_id) = dict.get("CFBundleIdentifier").and_then(|v| v.as_string()) else {
            continue;
        };

        if let Some(catalog_entry) = catalog_targets.get(bundle_id)
            && let Some(localized) = platform::read_spotlight_display_name(path_str, ".appex")
        {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if localized != catalog_entry.title && localized != stem {
                map.insert(bundle_id.to_string(), localized);
            }
        }
    }

    map
}

#[cfg(target_os = "macos")]
fn emit_entry_localized(
    tx: &mpsc::SyncSender<Candidate>,
    entry: &SettingsCatalogEntry,
    localized: &HashMap<String, String>,
) {
    let title = localized
        .get(entry.target)
        .map(|s| s.as_str())
        .unwrap_or(entry.title);
    let mut candidate = Candidate::new(
        &candidate_id(entry),
        CandidateKind::App,
        title,
        &target_path(entry),
    );
    candidate.subtitle = Some(subtitle(entry).into());
    let _ = tx.send(candidate);
}

#[cfg(target_os = "windows")]
fn emit_windows_control_panel_entries(tx: &mpsc::SyncSender<Candidate>) {
    for entry in platform::windows_control_panel_catalog() {
        let mut candidate = Candidate::new(
            &format!(
                "{SETTINGS_CANDIDATE_ID_PREFIX}{}",
                entry.candidate_id_suffix
            ),
            CandidateKind::App,
            entry.title,
            &platform::windows_control_panel_target_path(entry),
        );
        candidate.subtitle =
            Some(format!("{}{}", platform::settings_subtitle_prefix(), entry.aliases).into());
        let _ = tx.send(candidate);
    }
}

fn candidate_id(entry: &SettingsCatalogEntry) -> String {
    format!(
        "{SETTINGS_CANDIDATE_ID_PREFIX}{}",
        entry.candidate_id_suffix
    )
}

fn target_path(entry: &SettingsCatalogEntry) -> String {
    format!("{}{}", platform::settings_url_scheme_prefix(), entry.target)
}

fn subtitle(entry: &SettingsCatalogEntry) -> String {
    format!("{}{}", platform::settings_subtitle_prefix(), entry.aliases)
}

#[cfg(test)]
fn is_valid_target(target: &str) -> bool {
    target
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' || ch == ':')
}

#[cfg(test)]
fn is_valid_candidate_id_suffix(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use look_indexing::CandidateIdKind;
    use std::collections::HashSet;

    #[test]
    fn curated_settings_catalog_has_valid_fields() {
        let mut seen_targets = HashSet::new();
        let mut seen_candidate_id_suffixes = HashSet::new();
        let mut seen_titles = HashSet::new();
        let scheme = platform::settings_url_scheme_prefix();
        let subtitle_prefix = platform::settings_subtitle_prefix();

        for entry in platform::settings_catalog() {
            assert!(!entry.title.trim().is_empty(), "title must be non-empty");
            assert!(
                seen_titles.insert(entry.title.to_ascii_lowercase()),
                "duplicate title: {}",
                entry.title
            );

            assert!(!entry.target.trim().is_empty(), "target must be non-empty");
            assert!(
                is_valid_target(entry.target),
                "target has invalid chars: {}",
                entry.target
            );
            assert!(
                seen_targets.insert(entry.target.to_ascii_lowercase()),
                "duplicate target: {}",
                entry.target
            );

            assert!(
                !entry.candidate_id_suffix.trim().is_empty(),
                "candidate_id_suffix must be non-empty"
            );
            assert!(
                is_valid_candidate_id_suffix(entry.candidate_id_suffix),
                "candidate_id_suffix has invalid chars: {}",
                entry.candidate_id_suffix
            );
            assert!(
                seen_candidate_id_suffixes.insert(entry.candidate_id_suffix.to_ascii_lowercase()),
                "duplicate candidate_id_suffix: {}",
                entry.candidate_id_suffix
            );

            assert!(
                !entry.aliases.trim().is_empty(),
                "aliases must be non-empty"
            );
            assert!(
                entry.aliases.contains("settings"),
                "aliases should include settings hint: {}",
                entry.aliases
            );

            let prefixed_target = target_path(entry);
            assert!(prefixed_target.starts_with(scheme));
            assert!(
                subtitle(entry).starts_with(subtitle_prefix),
                "subtitle should include settings subtitle prefix"
            );
        }
    }

    #[test]
    fn discovery_outputs_valid_settings_candidates() {
        let (tx, rx) = mpsc::sync_channel(64);
        let producer = std::thread::spawn(move || {
            discover_system_settings_entries(tx);
        });
        let discovered: Vec<Candidate> = rx.into_iter().collect();
        producer.join().expect("settings discovery thread panicked");

        let expected_len = if platform::has_settings_app() {
            #[allow(unused_mut)]
            let mut total = platform::settings_catalog().len();
            #[cfg(target_os = "windows")]
            {
                total += platform::windows_control_panel_catalog().len();
            }
            total
        } else {
            // The only no-settings-app case is Linux, which still surfaces the
            // Bluetooth entry alone when a controller is present.
            #[cfg(target_os = "linux")]
            {
                usize::from(platform::bluetooth_present())
            }
            #[cfg(not(target_os = "linux"))]
            {
                0
            }
        };
        assert_eq!(discovered.len(), expected_len);

        let settings_scheme = platform::settings_url_scheme_prefix();
        for candidate in discovered {
            assert_eq!(candidate.kind, CandidateKind::App);
            assert!(candidate.id.starts_with(CandidateIdKind::PREFIX_SETTING));
            // Windows Control Panel entries use look-cmd:// instead of ms-settings:.
            let path_ok = candidate.path.starts_with(settings_scheme)
                || candidate.path.starts_with("look-cmd://");
            assert!(path_ok, "unexpected path scheme: {}", candidate.path);
            assert!(
                candidate
                    .subtitle
                    .as_deref()
                    .is_some_and(|s| { s.starts_with(platform::settings_subtitle_prefix()) })
            );
        }
    }
}
