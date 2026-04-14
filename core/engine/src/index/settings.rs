use crate::index::SETTINGS_CANDIDATE_ID_PREFIX;
use crate::platform;
use crate::platform::SettingsCatalogEntry;
use look_indexing::{Candidate, CandidateKind};
use std::sync::mpsc;

pub fn discover_system_settings_entries(tx: mpsc::SyncSender<Candidate>) {
    for entry in platform::settings_catalog() {
        let mut candidate = Candidate::new(
            &candidate_id(entry),
            CandidateKind::App,
            entry.title,
            &target_path(entry),
        );
        candidate.subtitle = Some(subtitle(entry).into());
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
        discover_system_settings_entries(tx);
        let discovered: Vec<Candidate> = rx.into_iter().collect();

        assert_eq!(discovered.len(), platform::settings_catalog().len());

        for candidate in discovered {
            assert_eq!(candidate.kind, CandidateKind::App);
            assert!(candidate.id.starts_with(CandidateIdKind::PREFIX_SETTING));
            assert!(
                candidate
                    .path
                    .starts_with(platform::settings_url_scheme_prefix())
            );
            assert!(
                candidate
                    .subtitle
                    .as_deref()
                    .is_some_and(|s| { s.starts_with(platform::settings_subtitle_prefix()) })
            );
        }
    }
}
