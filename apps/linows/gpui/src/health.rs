//! Setup problems the backend reports (a dead hotkey, a GNOME extension
//! waiting on a re-login), shown as the sticky banner until dismissed.
//! Dismissals persist in the state directory, keyed by id and kind rather
//! than message, so a problem is nagged about once per cause.

use std::collections::HashSet;
use std::path::PathBuf;

use linows_backend::health::HealthIssue;

const DISMISSED_FILE: &str = "health-dismissed";

fn dismissed_path() -> Option<PathBuf> {
    Some(linows_backend::crash::state_dir()?.join(DISMISSED_FILE))
}

fn key(issue: &HealthIssue) -> String {
    format!("{}:{}", issue.id, issue.kind)
}

fn dismissed() -> HashSet<String> {
    dismissed_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// The issues still worth showing, as one text.
pub fn notice(issues: &[HealthIssue]) -> Option<String> {
    let dismissed = dismissed();
    let lines: Vec<&str> = issues
        .iter()
        .filter(|issue| !dismissed.contains(&key(issue)))
        .map(|issue| issue.message.as_str())
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// Remember every current issue as seen. Best effort: a failed write means
/// the notice returns next launch.
pub fn dismiss(issues: &[HealthIssue]) {
    let Some(path) = dismissed_path() else {
        return;
    };
    let mut set = dismissed();
    set.extend(issues.iter().map(key));
    let mut keys: Vec<String> = set.into_iter().collect();
    keys.sort();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(err) = std::fs::write(&path, keys.join("\n")) {
        eprintln!("[health] remembering dismissals: {err}");
    }
}
