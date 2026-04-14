use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PathStyle {
    Posix,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PathPolicy {
    style: PathStyle,
    case_insensitive: bool,
}

fn runtime_policy() -> PathPolicy {
    if cfg!(target_os = "windows") {
        return PathPolicy {
            style: PathStyle::Windows,
            case_insensitive: true,
        };
    }

    PathPolicy {
        style: PathStyle::Posix,
        case_insensitive: false,
    }
}

fn policy_for_base(base: &str) -> PathPolicy {
    if looks_like_windows_absolute_path(base) || (base.contains('\\') && !base.contains('/')) {
        return PathPolicy {
            style: PathStyle::Windows,
            case_insensitive: true,
        };
    }
    runtime_policy()
}

fn separator_for_style(style: PathStyle) -> char {
    match style {
        PathStyle::Posix => '/',
        PathStyle::Windows => '\\',
    }
}

fn normalize_for_policy(path: &str, policy: PathPolicy) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }

    if policy.case_insensitive {
        return normalized.to_ascii_lowercase();
    }

    normalized
}

pub(crate) fn normalize_for_path_matching(path: &str) -> String {
    normalize_for_policy(path, runtime_policy())
}

pub(crate) fn path_is_same_or_child(path: &str, parent: &str) -> bool {
    let policy = runtime_policy();
    let normalized_path = normalize_for_policy(path, policy);
    let normalized_parent = normalize_for_policy(parent, policy);
    if normalized_parent.is_empty() {
        return false;
    }

    normalized_path == normalized_parent || normalized_path.starts_with(&(normalized_parent + "/"))
}

pub(crate) fn candidate_id_path_component(path: &str) -> String {
    normalize_for_path_matching(path).to_ascii_lowercase()
}

pub(crate) fn join_path(base: &str, child: &str) -> String {
    let policy = policy_for_base(base);
    let separator = separator_for_style(policy.style);
    let trimmed_base = base.trim_end_matches(['/', '\\']);
    let trimmed_child = child.trim_start_matches(['/', '\\']);

    if trimmed_base.is_empty() {
        return trimmed_child.to_string();
    }
    if trimmed_child.is_empty() {
        return trimmed_base.to_string();
    }

    format!("{trimmed_base}{separator}{trimmed_child}")
}

pub(crate) fn looks_like_absolute_path(path: &str) -> bool {
    path.starts_with('/') || Path::new(path).is_absolute() || looks_like_windows_absolute_path(path)
}

pub(crate) fn expand_with_home(value: &str, home: Option<&str>) -> String {
    if value.starts_with("~/") {
        return home
            .map(|prefix| join_path(prefix, value.trim_start_matches("~/")))
            .unwrap_or_else(|| value.to_string());
    }

    if looks_like_absolute_path(value) {
        return value.to_string();
    }

    home.map(|prefix| join_path(prefix, value))
        .unwrap_or_else(|| value.to_string())
}

fn looks_like_windows_absolute_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        return true;
    }

    path.starts_with("\\\\")
}

#[cfg(test)]
mod tests {
    use super::{expand_with_home, join_path, looks_like_absolute_path, path_is_same_or_child};

    #[test]
    fn absolute_path_check_supports_windows_drive_and_unc() {
        assert!(looks_like_absolute_path("/tmp"));
        assert!(looks_like_absolute_path("C:\\Windows\\System32"));
        assert!(looks_like_absolute_path("\\\\server\\share\\folder"));
    }

    #[test]
    fn path_boundary_matching_is_separator_aware() {
        assert!(path_is_same_or_child(
            "C:/Users/demo/Downloads",
            "C:\\Users\\demo"
        ));
        assert!(!path_is_same_or_child(
            "C:/Users/demo/Down",
            "C:/Users/demo/Downloads"
        ));
    }

    #[test]
    fn join_path_uses_separator_from_base_style() {
        assert_eq!(join_path("/Users/demo", "Projects"), "/Users/demo/Projects");
        assert_eq!(
            join_path("C:\\Users\\demo", "Projects"),
            "C:\\Users\\demo\\Projects"
        );
    }

    #[test]
    fn expand_with_home_handles_absolute_and_relative_inputs() {
        assert_eq!(
            expand_with_home("~/Projects", Some("/Users/demo")),
            "/Users/demo/Projects"
        );
        assert_eq!(
            expand_with_home("Documents", Some("/Users/demo")),
            "/Users/demo/Documents"
        );
        assert_eq!(expand_with_home("/tmp", Some("/Users/demo")), "/tmp");
    }
}
