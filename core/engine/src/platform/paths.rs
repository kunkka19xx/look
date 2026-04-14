use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PathStyle {
    Posix,
    Windows,
}

const WINDOWS_RESERVED_DEVICE_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PathPolicy {
    style: PathStyle,
}

impl PathPolicy {
    pub(crate) fn current() -> Self {
        if cfg!(target_os = "windows") {
            return Self {
                style: PathStyle::Windows,
            };
        }

        Self {
            style: PathStyle::Posix,
        }
    }

    pub(crate) fn for_base(base: &str) -> Self {
        if base.starts_with('/') || base.contains('/') {
            return Self {
                style: PathStyle::Posix,
            };
        }

        if looks_like_windows_absolute_path(base) || (base.contains('\\') && !base.contains('/')) {
            return Self {
                style: PathStyle::Windows,
            };
        }

        Self::current()
    }

    pub(crate) fn normalize_for_matching(&self, path: &str) -> String {
        let mut normalized = match self.style {
            PathStyle::Windows => path.replace('\\', "/"),
            PathStyle::Posix => path.to_string(),
        };
        while normalized.len() > 1 && normalized.ends_with('/') {
            normalized.pop();
        }

        if matches!(self.style, PathStyle::Windows) {
            return normalized.to_lowercase();
        }

        normalized
    }

    pub(crate) fn is_same_or_child(&self, path: &str, parent: &str) -> bool {
        let normalized_path = self.normalize_for_matching(path);
        let normalized_parent = self.normalize_for_matching(parent);
        if normalized_parent.is_empty() {
            return false;
        }

        normalized_path == normalized_parent
            || normalized_path.starts_with(&(normalized_parent + "/"))
    }

    pub(crate) fn join(&self, base: &str, child: &str) -> String {
        let separator = separator_for_style(self.style);
        let trimmed_base = base.trim_end_matches(['/', '\\']);
        let normalized_base = if trimmed_base.is_empty() {
            if base.starts_with('/') {
                "/"
            } else if base.starts_with('\\') {
                "\\"
            } else {
                ""
            }
        } else {
            trimmed_base
        };
        let trimmed_child = child.trim_start_matches(['/', '\\']);

        if normalized_base.is_empty() {
            return trimmed_child.to_string();
        }
        if trimmed_child.is_empty() {
            return normalized_base.to_string();
        }

        if normalized_base == "/" || normalized_base == "\\" {
            return format!("{normalized_base}{trimmed_child}");
        }

        format!("{normalized_base}{separator}{trimmed_child}")
    }

    pub(crate) fn is_valid_component(&self, name: &str) -> bool {
        is_valid_filename_component_for_style(name, self.style)
    }

    pub(crate) fn id_component(&self, path: &str) -> String {
        self.normalize_for_matching(path).to_lowercase()
    }
}

fn separator_for_style(style: PathStyle) -> char {
    match style {
        PathStyle::Posix => '/',
        PathStyle::Windows => '\\',
    }
}

fn is_valid_filename_component_for_style(name: &str, style: PathStyle) -> bool {
    if is_empty_or_relative_component(name) {
        return false;
    }

    if name.contains('/') {
        return false;
    }

    match style {
        PathStyle::Posix => true,
        PathStyle::Windows => {
            if name.contains('\\') {
                return false;
            }

            if name.ends_with(' ') || name.ends_with('.') {
                return false;
            }

            if name
                .chars()
                .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
            {
                return false;
            }

            !is_windows_reserved_name(name)
        }
    }
}

fn is_empty_or_relative_component(name: &str) -> bool {
    name.is_empty() || name == "." || name == ".."
}

fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    WINDOWS_RESERVED_DEVICE_NAMES
        .iter()
        .any(|reserved| reserved == &stem)
}

pub(crate) fn path_is_same_or_child(path: &str, parent: &str) -> bool {
    PathPolicy::current().is_same_or_child(path, parent)
}

pub(crate) fn candidate_id_path_component(path: &str) -> String {
    PathPolicy::current().id_component(path)
}

pub(crate) fn join_path(base: &str, child: &str) -> String {
    PathPolicy::for_base(base).join(base, child)
}

#[allow(dead_code)]
pub(crate) fn is_valid_filename_component(name: &str) -> bool {
    PathPolicy::current().is_valid_component(name)
}

#[allow(dead_code)]
pub(crate) fn is_valid_directory_component(name: &str) -> bool {
    is_valid_filename_component(name)
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
    #[cfg(target_os = "windows")]
    use super::path_is_same_or_child;
    use super::{PathPolicy, PathStyle, is_valid_filename_component_for_style};
    use super::{expand_with_home, join_path, looks_like_absolute_path};

    #[test]
    fn absolute_path_check_supports_windows_drive_and_unc() {
        assert!(looks_like_absolute_path("/tmp"));
        assert!(looks_like_absolute_path("C:\\Windows\\System32"));
        assert!(looks_like_absolute_path("\\\\server\\share\\folder"));
    }

    #[test]
    #[cfg(target_os = "windows")]
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
    fn posix_normalization_keeps_backslash_and_spaces() {
        let policy = PathPolicy {
            style: PathStyle::Posix,
        };

        assert_eq!(policy.normalize_for_matching("/tmp/a\\b "), "/tmp/a\\b ");
    }

    #[test]
    fn join_path_uses_separator_from_base_style() {
        assert_eq!(join_path("/Users/demo", "Projects"), "/Users/demo/Projects");
        assert_eq!(
            join_path("C:\\Users\\demo", "Projects"),
            "C:\\Users\\demo\\Projects"
        );
        assert_eq!(join_path("/", "Projects"), "/Projects");
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
        assert_eq!(expand_with_home("~/Desktop", Some("/")), "/Desktop");
    }

    #[test]
    fn filename_validation_is_platform_aware() {
        assert!(is_valid_filename_component_for_style(
            "report.md",
            PathStyle::Posix
        ));
        assert!(is_valid_filename_component_for_style(
            "name\\with-backslash",
            PathStyle::Posix
        ));
        assert!(!is_valid_filename_component_for_style(
            "name/with-slash",
            PathStyle::Posix
        ));

        assert!(is_valid_filename_component_for_style(
            "report.md",
            PathStyle::Windows
        ));
        assert!(!is_valid_filename_component_for_style(
            "name\\with-backslash",
            PathStyle::Windows
        ));
        assert!(!is_valid_filename_component_for_style(
            "name/with-slash",
            PathStyle::Windows
        ));
        assert!(!is_valid_filename_component_for_style(
            "CON",
            PathStyle::Windows
        ));
        assert!(!is_valid_filename_component_for_style(
            "file ",
            PathStyle::Windows
        ));
    }

    #[test]
    fn directory_component_validation_matches_filename_rules() {
        assert!(super::is_valid_directory_component("Desktop"));
        assert!(!super::is_valid_directory_component("nested/name"));
    }
}
