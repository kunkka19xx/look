//! Enumerate installed font families on Linux via `fc-list`.
//!
//! Fontconfig's `--format "%{family}\n"` returns a comma-separated list of
//! aliases per font (e.g. "Noto Sans,Noto Sans Display"); we split, trim,
//! dedup so the picker shows each family once.

pub(crate) fn list() -> Vec<String> {
    let Ok(output) = super::host_command("fc-list")
        .args(["--format", "%{family}\n"])
        .output()
    else {
        return Vec::new();
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut fonts: Vec<String> = stdout
        .lines()
        .flat_map(|line| line.split(',').map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect();
    fonts.sort();
    fonts.dedup();
    fonts
}
