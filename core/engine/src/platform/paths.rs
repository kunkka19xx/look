use std::path::Path;

pub(crate) fn normalize_for_path_matching(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while normalized.ends_with('/') {
        normalized.pop();
    }

    if cfg!(target_os = "windows") {
        return normalized.to_ascii_lowercase();
    }

    normalized
}

pub(crate) fn path_is_same_or_child(path: &str, parent: &str) -> bool {
    let normalized_path = normalize_for_path_matching(path);
    let normalized_parent = normalize_for_path_matching(parent);
    if normalized_parent.is_empty() {
        return false;
    }

    normalized_path == normalized_parent || normalized_path.starts_with(&(normalized_parent + "/"))
}

pub(crate) fn candidate_id_path_component(path: &str) -> String {
    normalize_for_path_matching(path).to_ascii_lowercase()
}

pub(crate) fn looks_like_absolute_path(path: &str) -> bool {
    Path::new(path).is_absolute() || looks_like_windows_absolute_path(path)
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
    use super::{looks_like_absolute_path, path_is_same_or_child};

    #[test]
    fn absolute_path_check_supports_windows_drive_and_unc() {
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
}
