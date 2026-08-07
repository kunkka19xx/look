//! Long-term memory: a small, bounded set of durable facts the user explicitly
//! asked the assistant to remember ("remember I prefer metric"). One JSON file
//! per the shell-supplied path, shared by every shell. Explicit-capture only -
//! nothing is stored without a `remember` command, and the model never writes
//! here (so a weak planner can't pollute it).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const MEMORY_LIMIT: usize = 50;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub id: String,
    pub text: String,
}

/// Stable content id so re-remembering the same fact dedups instead of piling up.
fn fact_id(text: &str) -> String {
    let norm = text.trim().to_lowercase();
    let mut hash: u64 = 0xcbf29ce484222325; // FNV-1a
    for b in norm.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn load(path: &Path) -> Vec<Fact> {
    let Ok(data) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn load_json(path: &Path) -> String {
    serde_json::to_string(&load(path)).unwrap_or_else(|_| "[]".into())
}

fn save(path: &Path, list: &[Fact]) -> bool {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_vec(list) {
        Ok(data) => fs::write(path, data).is_ok(),
        Err(_) => false,
    }
}

/// Store a fact (deduped by content, newest first). Returns the cleaned text, or
/// None when it is empty.
pub fn add(path: &Path, text: &str) -> Option<String> {
    let clean = text.trim();
    if clean.is_empty() {
        return None;
    }
    let fact = Fact { id: fact_id(clean), text: clean.to_string() };
    let mut list = load(path);
    list.retain(|f| f.id != fact.id);
    list.insert(0, fact);
    list.truncate(MEMORY_LIMIT);
    save(path, &list);
    Some(clean.to_string())
}

/// Remove facts matching `query` (a stored id, or a case-insensitive substring
/// of the text). Returns how many were removed.
pub fn forget(path: &Path, query: &str) -> usize {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return 0;
    }
    let mut list = load(path);
    let before = list.len();
    list.retain(|f| f.id != query && !f.text.to_lowercase().contains(&q));
    let removed = before - list.len();
    if removed > 0 {
        save(path, &list);
    }
    removed
}

/// The facts rendered as a model context block, or empty when there are none.
pub fn context_block(path: &Path) -> String {
    let list = load(path);
    if list.is_empty() {
        return String::new();
    }
    let mut out = String::from("The user has asked you to remember these facts about them:\n");
    for f in &list {
        out.push_str("- ");
        out.push_str(&f.text);
        out.push('\n');
    }
    out
}

pub enum Command {
    Remember(String),
    Forget(String),
    List,
}

/// Parse an explicit memory command from AI input (the `>` already consumed by
/// the shell). Returns None for normal AI input, so the shell falls through to
/// the planner/chat.
pub fn parse_command(input: &str) -> Option<Command> {
    let trimmed = input.trim();
    let lower = trimmed.to_lowercase();
    for prefix in ["remember that ", "remember "] {
        if lower.starts_with(prefix) {
            return Some(Command::Remember(trimmed[prefix.len()..].trim().to_string()));
        }
    }
    for prefix in ["forget that ", "forget "] {
        if lower.starts_with(prefix) {
            return Some(Command::Forget(trimmed[prefix.len()..].trim().to_string()));
        }
    }
    if lower == "memories" || lower == "what do you remember" || lower == "list memories" {
        return Some(Command::List);
    }
    None
}

/// Handle an input as a possible memory command: returns feedback for the shell
/// to show (and skip the model), or None when it is normal AI input.
pub fn handle_command(path: &Path, input: &str) -> Option<String> {
    match parse_command(input)? {
        Command::Remember(text) => Some(match add(path, &text) {
            Some(fact) => format!("Remembered: {fact}"),
            None => "Nothing to remember.".into(),
        }),
        Command::Forget(query) => {
            let n = forget(path, &query);
            Some(if n == 0 {
                format!("No memory matching \"{query}\".")
            } else {
                format!("Forgot {n} memor{}.", if n == 1 { "y" } else { "ies" })
            })
        }
        Command::List => {
            let list = load(path);
            Some(if list.is_empty() {
                "No memories yet. Say \"remember ...\" to add one.".into()
            } else {
                let lines: Vec<String> = list.iter().map(|f| format!("· {}", f.text)).collect();
                format!("Remembering:\n{}", lines.join("\n"))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("look-mem-test-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn add_dedups_and_caps() {
        let path = temp_file("caps");
        let _ = fs::remove_file(&path);
        add(&path, "I prefer metric");
        add(&path, "I prefer metric"); // same fact -> dedup
        assert_eq!(load(&path).len(), 1);
        for i in 0..MEMORY_LIMIT + 5 {
            add(&path, &format!("fact {i}"));
        }
        assert_eq!(load(&path).len(), MEMORY_LIMIT);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn forget_by_substring() {
        let path = temp_file("forget");
        let _ = fs::remove_file(&path);
        add(&path, "My dog is named Rex");
        add(&path, "I prefer metric");
        assert_eq!(forget(&path, "dog"), 1);
        assert_eq!(load(&path).len(), 1);
        assert_eq!(load(&path)[0].text, "I prefer metric");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn parse_and_handle_commands() {
        let path = temp_file("cmd");
        let _ = fs::remove_file(&path);
        assert_eq!(
            handle_command(&path, "remember that I am vegetarian").as_deref(),
            Some("Remembered: I am vegetarian")
        );
        assert!(matches!(parse_command("what's the weather"), None));
        assert!(context_block(&path).contains("vegetarian"));
        assert!(handle_command(&path, "memories").unwrap().contains("vegetarian"));
        assert_eq!(handle_command(&path, "forget vegetarian").as_deref(), Some("Forgot 1 memory."));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn normal_input_is_not_a_command() {
        assert!(parse_command("what should I cook tonight").is_none());
    }
}
