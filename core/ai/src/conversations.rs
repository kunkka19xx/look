//! Capped conversation store, one human-readable JSON file, shared by every
//! shell (the shell supplies the path, since app-data directories are
//! platform-specific). Upserts are incremental (quit-safe). `updated_at` is an
//! ISO-8601 UTC string, which sorts lexicographically.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const CONVERSATION_LIMIT: usize = 20;
pub const ITEM_LIMIT: usize = 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredItem {
    pub kind: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub items: Vec<StoredItem>,
}

pub fn load(path: &Path) -> Vec<Conversation> {
    let Ok(data) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn load_json(path: &Path) -> String {
    serde_json::to_string(&load(path)).unwrap_or_else(|_| "[]".into())
}

pub fn upsert(path: &Path, mut conversation: Conversation) -> bool {
    let items_len = conversation.items.len();
    if items_len > ITEM_LIMIT {
        conversation.items.drain(0..items_len - ITEM_LIMIT);
    }
    let mut list = load(path);
    list.retain(|existing| existing.id != conversation.id);
    list.insert(0, conversation);
    // ISO-8601 UTC sorts lexicographically; newest first.
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    list.truncate(CONVERSATION_LIMIT);

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_vec(&list) {
        Ok(data) => fs::write(path, data).is_ok(),
        Err(_) => false,
    }
}

pub fn upsert_json(path: &Path, conversation_json: &str) -> bool {
    match serde_json::from_str::<Conversation>(conversation_json) {
        Ok(conversation) => upsert(path, conversation),
        Err(_) => false,
    }
}

/// Remove one conversation by id. Returns whether it existed and was written.
pub fn delete(path: &Path, id: &str) -> bool {
    let mut list = load(path);
    let before = list.len();
    list.retain(|c| c.id != id);
    if list.len() == before {
        return false;
    }
    match serde_json::to_vec(&list) {
        Ok(data) => fs::write(path, data).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("look-ai-test-{name}-{}.json", std::process::id()))
    }

    fn convo(id: &str, updated_at: &str, items: usize) -> Conversation {
        Conversation {
            id: id.into(),
            title: format!("c{id}"),
            updated_at: updated_at.into(),
            items: (0..items)
                .map(|i| StoredItem { kind: "user".into(), text: format!("m{i}"), source: None })
                .collect(),
        }
    }

    #[test]
    fn upsert_replaces_caps_and_sorts() {
        let path = temp_file("caps");
        let _ = fs::remove_file(&path);

        for i in 0..25 {
            assert!(upsert(&path, convo(&i.to_string(), &format!("2026-01-{:02}T00:00:00Z", i + 1), 2)));
        }
        let list = load(&path);
        assert_eq!(list.len(), CONVERSATION_LIMIT);
        assert_eq!(list[0].id, "24"); // newest first

        // Replacing an existing id keeps one entry and re-sorts it to the top.
        assert!(upsert(&path, convo("10", "2026-02-01T00:00:00Z", 3)));
        let list = load(&path);
        assert_eq!(list[0].id, "10");
        assert_eq!(list.iter().filter(|c| c.id == "10").count(), 1);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn items_are_capped_keeping_the_tail() {
        let path = temp_file("items");
        let _ = fs::remove_file(&path);
        assert!(upsert(&path, convo("a", "2026-01-01T00:00:00Z", ITEM_LIMIT + 10)));
        let list = load(&path);
        assert_eq!(list[0].items.len(), ITEM_LIMIT);
        assert_eq!(list[0].items.last().unwrap().text, format!("m{}", ITEM_LIMIT + 9));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn delete_removes_by_id() {
        let path = temp_file("delete");
        let _ = fs::remove_file(&path);
        upsert(&path, convo("a", "2026-01-01T00:00:00Z", 1));
        upsert(&path, convo("b", "2026-01-02T00:00:00Z", 1));
        assert!(delete(&path, "a"));
        let list = load(&path);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "b");
        assert!(!delete(&path, "missing"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_or_garbage_file_loads_empty() {
        assert!(load(Path::new("/nonexistent/look-ai.json")).is_empty());
        let path = temp_file("garbage");
        fs::write(&path, "not json").unwrap();
        assert!(load(&path).is_empty());
        let _ = fs::remove_file(&path);
    }
}
