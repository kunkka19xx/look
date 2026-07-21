use crate::index::SETTINGS_CANDIDATE_ID_PREFIX;
use crate::platform;
use crate::platform::SettingsCatalogEntry;
use look_indexing::{Candidate, CandidateKind};
use std::collections::HashMap;
use std::sync::mpsc;

pub fn discover_system_settings_entries(
    localized_app_names: bool,
    tx: mpsc::SyncSender<Candidate>,
) {
    // With a settings app (gnome-control-center family), emit the whole catalog.
    // e.g. gnome-control-center on GNOME, skipped on i3/sway/minimal distros.
    if platform::has_settings_app() {
        let localized = localized_titles(localized_app_names);

        for entry in platform::settings_catalog() {
            emit_entry(&tx, entry, &display_title(entry, &localized));
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
        emit_entry(tx, entry, entry.title);
    }
}

/// Only Linux has a fallback: elsewhere a missing settings app means there is
/// nothing to surface. Kept as a real function so the branch above reads the same
/// on every platform, rather than an early return that dangles once the Linux-only
/// tail is compiled out.
#[cfg(not(target_os = "linux"))]
fn emit_settings_fallback_entries(_tx: &mpsc::SyncSender<Candidate>) {}

fn emit_entry(tx: &mpsc::SyncSender<Candidate>, entry: &SettingsCatalogEntry, title: &str) {
    let mut candidate = Candidate::new(
        &candidate_id(entry),
        CandidateKind::App,
        title,
        &target_path(entry),
    );
    candidate.subtitle = Some(subtitle(entry).into());
    let _ = tx.send(candidate);
}

/// Localized pane names, keyed by `SettingsCatalogEntry::target`. Only macOS
/// has a source for these, so everywhere else this stays empty and the catalog
/// titles are used verbatim.
type LocalizedTitles = HashMap<&'static str, String>;

fn localized_titles(localized_app_names: bool) -> LocalizedTitles {
    if !localized_app_names {
        return LocalizedTitles::new();
    }
    #[cfg(target_os = "macos")]
    {
        platform::localized_settings_titles(platform::settings_catalog())
    }
    #[cfg(not(target_os = "macos"))]
    {
        LocalizedTitles::new()
    }
}

/// Renders a pane as `<localized> (<English>)` when the two differ.
///
/// Keeping the catalog title in the *title* is what makes the English name
/// stay searchable: settings subtitles are only scored for settings-shaped
/// queries, so a title that dropped the English name would make the pane
/// unreachable by queries like `wifi` on a non-English system.
fn display_title(entry: &SettingsCatalogEntry, localized: &LocalizedTitles) -> String {
    match localized.get(entry.target) {
        Some(localized_title) => format!("{localized_title} ({})", entry.title),
        None => entry.title.to_string(),
    }
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
        assert_valid_settings_candidates(discover_settings(false));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn localized_discovery_outputs_valid_settings_candidates() {
        assert_valid_settings_candidates(discover_settings(true));
    }

    /// The English catalog title has to survive localization, otherwise queries
    /// like `wifi` stop reaching the pane on a non-English system.
    #[test]
    fn localized_title_keeps_the_english_catalog_title() {
        // "Bluetooth" in Simplified Chinese.
        const LOCALIZED_NAME: &str = "蓝牙";
        let entry = &platform::settings_catalog()[0];
        let localized = LocalizedTitles::from([(entry.target, LOCALIZED_NAME.to_string())]);

        let title = display_title(entry, &localized);

        assert!(title.contains(LOCALIZED_NAME), "missing localized name");
        assert!(
            title.contains(entry.title),
            "missing English catalog title: {title}"
        );
    }

    /// End-to-end guard for the same thing, through the real query engine:
    /// settings subtitles are only scored for settings-shaped queries, so the
    /// pane is reachable by its English name only while that name is in the
    /// title. `wifi` is not one of the settings query hints, which makes it the
    /// case that regressed when the title was replaced outright.
    #[cfg(target_os = "macos")]
    #[test]
    fn english_query_still_reaches_a_localized_pane() {
        // "Wi-Fi" in Simplified Chinese, as System Settings shows it.
        const LOCALIZED_WIFI: &str = "无线局域网";
        let entry = platform::settings_catalog()
            .iter()
            .find(|entry| entry.title == "Wi-Fi")
            .expect("catalog has a Wi-Fi pane");
        let localized = LocalizedTitles::from([(entry.target, LOCALIZED_WIFI.to_string())]);

        let (tx, rx) = mpsc::sync_channel(1);
        emit_entry(&tx, entry, &display_title(entry, &localized));
        drop(tx);
        let engine = crate::QueryEngine::new(rx.into_iter().collect());

        assert_eq!(engine.search("wifi", 5).len(), 1, "English query lost it");
        assert_eq!(
            engine.search(LOCALIZED_WIFI, 5).len(),
            1,
            "localized query lost it"
        );
    }

    /// No localized entry means no decoration: the title stays exactly what the
    /// catalog says on every platform.
    #[test]
    fn untranslated_title_is_the_catalog_title_verbatim() {
        let entry = &platform::settings_catalog()[0];
        let localized = localized_titles(false);

        assert_eq!(display_title(entry, &localized), entry.title);
    }

    fn discover_settings(localized_app_names: bool) -> Vec<Candidate> {
        let (tx, rx) = mpsc::sync_channel(64);
        let producer = std::thread::spawn(move || {
            discover_system_settings_entries(localized_app_names, tx);
        });
        let discovered = rx.into_iter().collect();
        producer.join().expect("settings discovery thread panicked");

        discovered
    }

    fn assert_valid_settings_candidates(discovered: Vec<Candidate>) {
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
