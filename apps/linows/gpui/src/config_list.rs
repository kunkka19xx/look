//! The backend's CSV contract for config lists (`app_exclude_names`,
//! `file_scan_extra_roots`, `file_exclude_paths`): `\,` is a literal comma
//! and `\\` a literal backslash.

pub fn parse(value: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' if matches!(chars.peek(), Some(',') | Some('\\')) => {
                current.push(chars.next().unwrap_or_default());
            }
            ',' => {
                let entry = current.trim();
                if !entry.is_empty() {
                    entries.push(entry.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let entry = current.trim();
    if !entry.is_empty() {
        entries.push(entry.to_string());
    }
    entries
}

pub fn render(entries: &[String]) -> String {
    entries
        .iter()
        .map(|e| e.replace('\\', "\\\\").replace(',', "\\,"))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_lists_round_trip_their_escapes() {
        let names = vec!["Zed".to_string(), "a,b".to_string(), "c\\d".to_string()];
        let rendered = render(&names);
        assert_eq!(rendered, "Zed,a\\,b,c\\\\d");
        assert_eq!(parse(&rendered), names);
        assert_eq!(parse(" , x ,"), vec!["x".to_string()]);
    }
}
