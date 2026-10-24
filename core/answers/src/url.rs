//! URL classification for launcher input.
//!
//! Decides whether a raw query is something the user typed *as a web address*
//! rather than a search term, and if so resolves it to an openable URL. Pure and
//! network-free - both platform shells (macOS FFI, linows Tauri) call this so the
//! rules and the risky TLD/extension list live once.
//!
//! Two tiers, by how certain the match is (see `UrlTier`). The tier exists so the
//! UI can rank a certain match on top while keeping a fuzzy bare-host match from
//! ever stealing the default selection away from a real local file/app/folder.

use serde::Serialize;

/// How confident we are that the query is a URL, which decides how aggressively
/// the UI may rank the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UrlTier {
    /// Structurally unambiguous: a scheme, a port, a path, localhost, or a full
    /// IPv4 address. A filename or plain search cannot resemble these, so the UI
    /// may rank it top.
    Structural,
    /// A bare `host.tld` on the gTLD allowlist. Plausible but not certain (it can
    /// collide with a filename), so the UI must not let it take the default slot
    /// while any local result exists.
    BareHost,
}

/// A resolved URL match: the openable address and how certain the classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UrlMatch {
    pub url: String,
    pub tier: UrlTier,
}

/// Curated web gTLDs that are never source-code file extensions, so a bare
/// `host.tld` on this list is safe to offer as a URL. Deliberately excludes
/// two-letter ccTLDs that double as extensions (`rs`, `py`, `sh`, `md`, `ml`,
/// `pl`, ...): `io`/`ai`/`co` are kept because they are not code extensions.
const GTLD_ALLOWLIST: &[&str] = &[
    "com", "org", "net", "io", "dev", "app", "ai", "co", "xyz", "info", "page",
];

/// Classify `query` as a URL, or `None` to leave it as a search term.
pub fn classify_url(query: &str) -> Option<UrlMatch> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    // Interior whitespace means a search phrase, not an address. A launcher
    // prefix marker (`a"`, `f"`, `t"`, ...) always carries a `"`; reject both.
    if q.chars().any(char::is_whitespace) || q.contains('"') {
        return None;
    }

    // Tier 1a: explicit scheme is taken as written.
    let lower = q.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    {
        if rest.is_empty() {
            return None;
        }
        return Some(UrlMatch {
            url: q.to_string(),
            tier: UrlTier::Structural,
        });
    }

    // Split authority (host[:port]) from an optional path.
    let (authority, has_path) = match q.find('/') {
        Some(i) => (&q[..i], true),
        None => (q, false),
    };
    let (host, port) = match authority.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    if host.is_empty() {
        return None;
    }
    // A port, when present, must be all digits.
    if port.is_some_and(|p| !is_valid_port(p)) {
        return None;
    }

    // Tier 1b: localhost or an IPv4 literal, always http://.
    if host.eq_ignore_ascii_case("localhost") {
        return Some(UrlMatch {
            url: format!("http://{q}"),
            tier: UrlTier::Structural,
        });
    }
    if let Some(parts) = ipv4_parts(host) {
        // One dot is a decimal unless a port says otherwise: `3.14` and
        // `183.10/2` stay numbers, `127.1:8080` is an address. A path is no
        // signal here, since `183.10/2` is a division. `http://127.1` already
        // returned above.
        if parts == 2 && port.is_none() {
            return None;
        }
        // Two dots can't be a decimal, but `1.2.3` can be a version folder, so
        // without a port or path it must not take the top slot. A full address
        // is certain.
        let tier = if parts == 4 || port.is_some() || has_path {
            UrlTier::Structural
        } else {
            UrlTier::BareHost
        };
        return Some(UrlMatch {
            url: format!("http://{q}"),
            tier,
        });
    }

    // Otherwise it must look like host.tld with sane labels.
    if host.split('.').any(|label| {
        label.is_empty() || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    }) {
        return None;
    }
    let tld = host.rsplit('.').next()?;
    // No dot at all, or a non-alphabetic TLD, is not a host we open.
    if tld == host || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    // Tier 1c: a port or a path is a filename/search cannot have; rank as certain.
    if has_path || port.is_some() {
        return Some(UrlMatch {
            url: format!("https://{q}"),
            tier: UrlTier::Structural,
        });
    }

    // Tier 2: bare host.tld only when the TLD is on the safe allowlist.
    if GTLD_ALLOWLIST.contains(&tld.to_ascii_lowercase().as_str()) {
        return Some(UrlMatch {
            url: format!("https://{q}"),
            tier: UrlTier::BareHost,
        });
    }
    None
}

/// The number of parts in an IPv4 literal, in the shorthand `inet_aton`
/// accepts: `127.1`, `127.0.1` and `127.0.0.1` are all 127.0.0.1. The last part
/// fills the remaining bytes, so its ceiling depends on how many came before.
/// Decimal only; nobody types hex or octal octets into a launcher.
fn ipv4_parts(host: &str) -> Option<usize> {
    let parts: Vec<&str> = host.split('.').collect();
    let last_max: u64 = match parts.len() {
        2 => 0xFF_FFFF,
        3 => 0xFFFF,
        4 => 0xFF,
        _ => return None,
    };
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let max = if i + 1 == parts.len() { last_max } else { 0xFF };
        if part.parse::<u64>().ok()? > max {
            return None;
        }
    }
    Some(parts.len())
}

/// A URL port is one or more ASCII digits.
fn is_valid_port(port: &str) -> bool {
    !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(q: &str) -> Option<(String, UrlTier)> {
        classify_url(q).map(|m| (m.url, m.tier))
    }

    #[test]
    fn resolves_and_tiers_urls() {
        assert_eq!(
            url("github.com"),
            Some(("https://github.com".into(), UrlTier::BareHost))
        );
        assert_eq!(
            url("github.com/pulls"),
            Some(("https://github.com/pulls".into(), UrlTier::Structural))
        );
        assert_eq!(
            url("localhost:3000"),
            Some(("http://localhost:3000".into(), UrlTier::Structural))
        );
        assert_eq!(
            url("127.0.0.1:8080"),
            Some(("http://127.0.0.1:8080".into(), UrlTier::Structural))
        );
        assert_eq!(
            url("https://x.com"),
            Some(("https://x.com".into(), UrlTier::Structural))
        );
        assert_eq!(
            url("example.com:8443"),
            Some(("https://example.com:8443".into(), UrlTier::Structural))
        );
    }

    #[test]
    fn ipv4_shorthand_needs_a_signal_a_decimal_cannot_give() {
        let structural = |q: &str, u: &str| {
            assert_eq!(url(q), Some((u.into(), UrlTier::Structural)), "{q}");
        };
        structural("192.168.1.1", "http://192.168.1.1");
        structural("127.1:8080", "http://127.1:8080");
        structural("127.0.1:3000", "http://127.0.1:3000");
        structural("1.2.3/notes", "http://1.2.3/notes");
        structural("http://127.1", "http://127.1");
        // Two dots is no decimal, but may be a version folder.
        assert_eq!(
            url("127.0.1"),
            Some(("http://127.0.1".into(), UrlTier::BareHost))
        );
        assert_eq!(
            url("1.2.3"),
            Some(("http://1.2.3".into(), UrlTier::BareHost))
        );
    }

    #[test]
    fn decimals_are_not_addresses() {
        for q in [
            "0.5",
            "3.14",
            "999.1",
            "127.1",
            "183.10/2",
            "2.5/2",
            "1.2.99999",
            "999.1.1.1",
            "1.2.3.4.5",
        ] {
            assert_eq!(url(q), None, "{q}");
        }
    }

    #[test]
    fn rejects_files_and_searches() {
        // Source-code extensions that are also real ccTLDs must stay files.
        assert_eq!(url("main.rs"), None);
        assert_eq!(url("readme.md"), None);
        assert_eq!(url("main.py"), None);
        // Non-TLD extensions.
        assert_eq!(url("main.go"), None);
        // Plain searches.
        assert_eq!(url("look up react docs"), None);
        assert_eq!(url("ratio 3:2"), None);
        // Launcher prefix markers.
        assert_eq!(url("f\"foo"), None);
        // Partial / malformed hosts.
        assert_eq!(url("github."), None);
        assert_eq!(url(".com"), None);
        assert_eq!(url("http://"), None);
        assert_eq!(url(""), None);
        assert_eq!(url("   "), None);
    }
}
