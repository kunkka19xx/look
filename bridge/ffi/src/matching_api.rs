//! C-ABI wrapper over `look_matching::fuzzy_score` so macOS ranks identically to
//! linows. The numeric port/PID tiers are process-finder specific and layered on
//! top natively in Swift, not here.

use crate::state::cstr_to_string;
use std::os::raw::c_char;

/// No-match sentinel. `fuzzy_score` never returns a negative score, so `i64::MIN`
/// is unambiguous; the Swift wrapper maps it back to `nil`.
pub(crate) const NO_MATCH: i64 = i64::MIN;

/// `look_matching::fuzzy_score`, adapted to the C ABI. Both arguments must be
/// pre-lowercased by the caller (`fuzzy_score` is case-sensitive).
pub(crate) fn look_fuzzy_score_impl(query: *const c_char, title: *const c_char) -> i64 {
    let query = cstr_to_string(query);
    let title = cstr_to_string(title);
    look_matching::fuzzy_score(&query, &title).unwrap_or(NO_MATCH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn score(query: &str, title: &str) -> i64 {
        let q = CString::new(query).unwrap();
        let t = CString::new(title).unwrap();
        look_fuzzy_score_impl(q.as_ptr(), t.as_ptr())
    }

    #[test]
    fn exact_and_prefix_and_subsequence_match() {
        assert_eq!(score("ghostty", "ghostty"), 2_000, "exact match");
        assert!(score("firefox", "firefox gpu helper") > 0, "prefix match");
        assert!(
            score("firefox", ".firefox-old") > 0,
            "substring/subsequence"
        );
        assert_eq!(score("zzq", "ghostty"), NO_MATCH, "no match -> sentinel");
    }

    #[test]
    fn null_pointers_do_not_crash() {
        assert_eq!(
            look_fuzzy_score_impl(std::ptr::null(), std::ptr::null()),
            2_000,
            "two empty strings compare equal"
        );
    }
}
