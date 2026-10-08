//! The release check behind "Check for Updates": GitHub's latest release
//! against the running version. The webview fetched this from JS; here it
//! goes through the answers crate's HTTP path, off the UI thread. A
//! dismissed version is remembered in the state dir so it does not come
//! back on the next automatic check, while a manual check still shows it.

use crate::crash::state_dir;
use look_answers::http;

const RELEASES_API_URL: &str = "https://api.github.com/repos/kunkka19xx/look/releases/latest";
const FALLBACK_RELEASES_URL: &str = "https://github.com/kunkka19xx/look/releases/latest";
const GITHUB_PREFIX: &str = "https://github.com/";
const TIMEOUT_SECS: u32 = 15;
const DISMISSED_FILE: &str = "update-dismissed";

pub const SCOOP_UPDATE_COMMAND: &str = "scoop update look";
pub const SELF_UPDATE_INSTALL_METHOD: &str = "nsis";
pub const SCOOP_INSTALL_METHOD: &str = "scoop";
/// Linux and Windows each have several install paths; the README section
/// has the command for each.
pub const INSTALL_HINT_URL: &str = if cfg!(windows) {
    "https://github.com/kunkka19xx/look#windows"
} else {
    "https://github.com/kunkka19xx/look#linux"
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    Available(Release),
    Latest,
    /// Newer, but the user dismissed this very version.
    Dismissed,
    Failed,
}

/// Ask GitHub. `force` is a manual check, which overrides a dismissal.
pub fn check(current: &str, force: bool) -> Check {
    let Some(json) = http::get_json(RELEASES_API_URL, TIMEOUT_SECS) else {
        return Check::Failed;
    };
    let Some(tag) = json.get("tag_name").and_then(|v| v.as_str()) else {
        return Check::Failed;
    };
    let flagged = |key: &str| json.get(key).and_then(|v| v.as_bool()) == Some(true);
    if flagged("draft") || flagged("prerelease") {
        return Check::Latest;
    }
    let latest = normalize(tag);
    if !is_newer(&latest, &normalize(current)) {
        return Check::Latest;
    }
    // Only GitHub's own address goes to the browser: a bad response must
    // not hand `xdg-open` an arbitrary scheme.
    let url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .filter(|u| u.starts_with(GITHUB_PREFIX))
        .unwrap_or(FALLBACK_RELEASES_URL)
        .to_string();
    if !force && dismissed().as_deref() == Some(latest.as_str()) {
        return Check::Dismissed;
    }
    Check::Available(Release {
        version: latest,
        url,
    })
}

/// Remember `version` as not wanted. Best effort: a failed write means the
/// banner returns next launch.
pub fn dismiss(version: &str) {
    if let Some(path) = dismissed_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, version);
    }
}

fn dismissed_path() -> Option<std::path::PathBuf> {
    Some(state_dir()?.join(DISMISSED_FILE))
}

fn dismissed() -> Option<String> {
    let text = std::fs::read_to_string(dismissed_path()?).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// `v1.2.3` and `1.2.3` are the same release.
fn normalize(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed
        .strip_prefix(['v', 'V'])
        .unwrap_or(trimmed)
        .to_string()
}

/// Dotted numeric compare: `1.10.0` is newer than `1.9.0`. A part that is
/// not a number counts as zero.
pub fn is_newer(lhs: &str, rhs: &str) -> bool {
    let a = components(lhs);
    let b = components(rhs);
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

fn components(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| {
            part.trim()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_text() {
        assert!(is_newer("1.10.0", "1.9.0"));
        assert!(!is_newer("1.9.0", "1.10.0"));
        assert!(!is_newer("0.7.2", "0.7.2"));
        assert!(is_newer("0.7.2.1", "0.7.2"));
        assert!(is_newer("1.0", "0.9.9"));
    }

    #[test]
    fn tags_lose_their_v() {
        assert_eq!(normalize("v0.7.2"), "0.7.2");
        assert_eq!(normalize(" V1.0 "), "1.0");
        assert_eq!(normalize("0.7.2"), "0.7.2");
    }
}
