//! Extract an app version on Linux by canonicalizing the binary's path and
//! pulling the version segment out of a `/nix/store/<hash>-<name>-<version>/`
//! path. Returns None on non-Nix distros — those don't have a stable,
//! cross-distro version string we can derive from the `Exec=` field alone.

pub(crate) fn read(path: &str) -> Option<String> {
    let bin = path.split_whitespace().next()?;

    let resolved = if bin.starts_with('/') {
        std::fs::canonicalize(bin).ok()
    } else {
        resolve_in_path(bin).and_then(|p| std::fs::canonicalize(p).ok())
    };

    let real = resolved?;
    let real_str = real.to_string_lossy();
    extract_nix_version(&real_str)
}

fn resolve_in_path(bin: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    for dir in path_var.split(':') {
        let candidate = std::path::Path::new(dir).join(bin);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn extract_nix_version(path: &str) -> Option<String> {
    let store_prefix = "/nix/store/";
    let rest = path.strip_prefix(store_prefix)?;
    let dir_part = rest.split('/').next()?;
    // 32-char hash + '-' = 33 prefix bytes; everything after is "<name>-<version>".
    let after_hash = dir_part.get(33..)?;
    let mut version_start = None;
    for (i, _) in after_hash.match_indices('-') {
        if after_hash
            .get(i + 1..i + 2)
            .map(|c| c.chars().next().unwrap_or(' ').is_ascii_digit())
            .unwrap_or(false)
        {
            version_start = Some(i + 1);
        }
    }
    let start = version_start?;
    Some(after_hash[start..].to_string())
}
