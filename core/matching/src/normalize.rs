//! Folding text so a search matches what a reader means.
//!
//! Lives in the matching crate, not the engine, because more than search needs
//! it: the AI tiers match a meeting title and a contact name against typed
//! text, and folding differently there would mean `hop` finds a FILE called
//! `Họp` while `join hop` finds nothing.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Lowercased, with diacritics stripped: `Họp` and `hop` compare equal, as do
/// `Café` and `cafe`.
///
/// Vietnamese needs one rule Unicode does not give for free: `đ` is a letter in
/// its own right rather than a `d` with a mark, so NFKD leaves it alone and it
/// has to be mapped by hand. Without that, `dien thoai` misses `Điện thoại`.
pub fn normalize_for_search(input: &str) -> String {
    // Fast path: pure ASCII avoids Unicode NFKD overhead
    if input.is_ascii() {
        let mut out = input.to_owned();
        out.make_ascii_lowercase();
        return out;
    }

    let mut out = String::with_capacity(input.len());

    for ch in input.nfkd() {
        if is_combining_mark(ch) {
            continue;
        }

        match ch {
            'đ' | 'Đ' => out.push('d'),
            _ => out.extend(ch.to_lowercase()),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_just_lowercased() {
        assert_eq!(normalize_for_search("Safari"), "safari");
        assert_eq!(normalize_for_search("main.go"), "main.go");
    }

    #[test]
    fn vietnamese_folds_to_what_people_type() {
        assert_eq!(normalize_for_search("Họp"), "hop");
        assert_eq!(normalize_for_search("Điện thoại"), "dien thoai");
        assert_eq!(normalize_for_search("Đà Nẵng"), "da nang");
    }

    #[test]
    fn other_latin_marks_fold_too() {
        assert_eq!(normalize_for_search("Café"), "cafe");
        assert_eq!(normalize_for_search("Müller"), "muller");
        assert_eq!(normalize_for_search("RÉSUMÉ"), "resume");
    }

    #[test]
    fn scripts_without_case_or_marks_pass_through() {
        // Han and kana have nothing to fold; the point is that they survive.
        assert_eq!(normalize_for_search("会议"), "会议");
        assert_eq!(normalize_for_search("いぬ"), "いぬ");
    }
}
