//! Target-language parsing for the translation panel, shared by every shell.
//!
//! The label table doubles as the allowlist: a code with no entry is a typo or
//! an unsupported language and is dropped. Each shell reads the raw
//! `translate_languages` value from the config and hands it here, so the parsing
//! and the labels live once instead of once per shell.
//!
//! The table is deliberately short. Every target costs one request per query,
//! and the MyMemory daily character quota is per IP and shared across all of
//! them, so breadth here is paid for in quota rather than in bytes.

use serde::Serialize;

/// A validated translation target: the canonical lowercase code and its display
/// label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TranslateLanguage {
    pub code: &'static str,
    pub label: &'static str,
}

/// The built-in languages, and the allowlist. Codes are lowercase because the
/// config value is lowercased before lookup (so `zh-CN` reads as `zh-cn`).
const LANGUAGES: &[(&str, &str)] = &[
    ("ar", "العربية"),
    ("bn", "বাংলা"),
    ("de", "Deutsch"),
    ("en", "English"),
    ("es", "Español"),
    ("fa", "فارسی"),
    ("fr", "Français"),
    ("hi", "हिन्दी"),
    ("id", "Bahasa Indonesia"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("nl", "Nederlands"),
    ("pl", "Polski"),
    ("pt", "Português"),
    ("pt-br", "Português (Brasil)"),
    ("ru", "Русский"),
    ("sv", "Svenska"),
    ("th", "ไทย"),
    ("tr", "Türkçe"),
    ("uk", "Українська"),
    ("vi", "Tiếng Việt"),
    ("zh", "中文"),
    ("zh-cn", "中文（简体）"),
    ("zh-tw", "中文（繁體）"),
];

/// The fallback when the config key is absent, empty, or entirely invalid.
const DEFAULTS: &[&str] = &["vi", "en", "ja"];

/// How many targets a query may fan out to. Each one is a separate request, and
/// the MyMemory quota is shared, so a longer list spends the day's budget
/// proportionally faster. Extras are dropped rather than refused: a too-long
/// list is still a usable one.
pub(crate) const MAX_TARGETS: usize = 3;
const _: () = assert!(DEFAULTS.len() <= MAX_TARGETS, "defaults exceed the cap");

fn lookup(code: &str) -> Option<TranslateLanguage> {
    LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|&(code, label)| TranslateLanguage { code, label })
}

/// Parses the raw `translate_languages` config value into the ordered, validated
/// target languages, at most `MAX_TARGETS` of them. Empty entries, duplicates,
/// and unknown codes are dropped; an absent, empty, or all-invalid list falls
/// back to the defaults, so a typo never blanks the panel. The result is never
/// empty, which is what lets the shells index the first entry.
pub fn translate_languages(raw: &str) -> Vec<TranslateLanguage> {
    let mut seen = std::collections::HashSet::new();
    let mut languages: Vec<TranslateLanguage> = raw
        .split(',')
        .filter_map(|entry| {
            let code = entry.trim().to_ascii_lowercase();
            if code.is_empty() || !seen.insert(code.clone()) {
                return None;
            }
            lookup(&code)
        })
        .collect();
    if languages.is_empty() {
        languages = DEFAULTS.iter().filter_map(|code| lookup(code)).collect();
    }
    languages.truncate(MAX_TARGETS);
    languages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_and_drops_unknown_or_duplicate_codes() {
        let languages = translate_languages("fr, vi, FR, nope, ja");
        assert_eq!(
            languages.iter().map(|l| l.code).collect::<Vec<_>>(),
            vec!["fr", "vi", "ja"]
        );
        assert_eq!(languages[1].label, "Tiếng Việt");
    }

    #[test]
    fn lowercases_and_trims_entries() {
        let languages = translate_languages("  ZH-CN ,  En ");
        assert_eq!(
            languages.iter().map(|l| l.code).collect::<Vec<_>>(),
            vec!["zh-cn", "en"]
        );
    }

    #[test]
    fn falls_back_to_defaults_when_absent_empty_or_all_invalid() {
        let defaults = vec!["vi", "en", "ja"];
        for raw in ["", "   ", ",,", "nope,also-nope"] {
            let codes: Vec<_> = translate_languages(raw).iter().map(|l| l.code).collect();
            assert_eq!(codes, defaults, "raw={raw:?}");
        }
    }

    #[test]
    fn a_single_code_resolves_to_its_label() {
        let languages = translate_languages("ja");
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].label, "日本語");
    }

    #[test]
    fn keeps_at_most_the_target_cap() {
        let languages = translate_languages("de,fr,es,it,nl");
        assert_eq!(
            languages.iter().map(|l| l.code).collect::<Vec<_>>(),
            vec!["de", "fr", "es"]
        );
    }

    #[test]
    fn every_default_is_in_the_table() {
        for code in DEFAULTS {
            assert!(lookup(code).is_some(), "missing default {code}");
        }
    }
}
