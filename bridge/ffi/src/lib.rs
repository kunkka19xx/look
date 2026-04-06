#![allow(unsafe_code)]

mod clipboard_api;
mod runtime_config;
mod search_api;
mod state;
mod translate_api;
mod usage_api;

use look_engine::QueryEngine;
use search_api::FfiSearchResult;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn look_search_count(query_len: u32) -> FfiSearchResult {
    search_api::look_search_count_impl(query_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_search_json(query: *const c_char, limit: u32) -> *mut c_char {
    search_api::look_search_json_impl(query, limit)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_record_usage(candidate_id: *const c_char, action: *const c_char) -> bool {
    usage_api::look_record_usage_impl(candidate_id, action)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_reload_config() -> bool {
    runtime_config::reload_runtime_config();
    let path = state::default_db_path();
    if QueryEngine::bootstrap_sqlite(&path).is_err() {
        return false;
    }
    state::refresh_engine_cache();
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn look_free_cstring(ptr: *mut c_char) {
    state::free_json_allocation(ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_translate_json(
    text: *const c_char,
    target_lang: *const c_char,
) -> *mut c_char {
    translate_api::look_translate_json_impl(text, target_lang)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_clipboard_store(
    content: *const c_char,
    content_type: *const c_char,
    source_app: *const c_char,
) -> bool {
    clipboard_api::look_clipboard_store_impl(content, content_type, source_app)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_clipboard_search(
    query: *const c_char,
    content_type: *const c_char,
    limit: u32,
) -> *mut c_char {
    clipboard_api::look_clipboard_search_impl(query, content_type, limit)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_clipboard_delete(item_id: *const c_char) -> bool {
    clipboard_api::look_clipboard_delete_impl(item_id)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_clipboard_clear(older_than_seconds: i64) -> u64 {
    clipboard_api::look_clipboard_clear_impl(older_than_seconds)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_clipboard_toggle_pin(item_id: *const c_char) -> bool {
    clipboard_api::look_clipboard_toggle_pin_impl(item_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use look_indexing::{Candidate, CandidateKind};
    use look_storage::SqliteStore;
    use std::env;
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn ffi_search_and_record_usage_smoke() {
        let lock = TEST_MUTEX.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().expect("test lock poisoned");

        let db_path = unique_test_db_path();
        let _ = fs::remove_file(&db_path);

        let mut store = SqliteStore::open(&db_path).expect("open sqlite store");
        let candidate = Candidate {
            id: "app:smoke.test".to_string(),
            kind: CandidateKind::App,
            title: "Smoke Test App".to_string(),
            subtitle: Some("smoke app".to_string()),
            path: "/Applications/Smoke Test App.app".to_string(),
            use_count: 0,
            last_used_at_unix_s: None,
        };
        store
            .upsert_candidates(&[candidate])
            .expect("insert smoke candidate");

        unsafe {
            env::set_var("LOOK_DB_PATH", db_path.as_os_str());
        }

        let query = CString::new("smoke").expect("query cstring");
        let ptr = look_search_json(query.as_ptr(), 10);
        assert!(!ptr.is_null());

        let raw = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(ptr);

        let payload: serde_json::Value = serde_json::from_str(&raw).expect("valid search payload");
        let count = payload
            .get("count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        assert!(count >= 1);

        let has_smoke = payload
            .get("results")
            .and_then(|value| value.as_array())
            .is_some_and(|results| {
                results.iter().any(|item| {
                    item.get("id")
                        .and_then(|value| value.as_str())
                        .is_some_and(|id| id == "app:smoke.test")
                })
            });
        assert!(has_smoke);

        let id = CString::new("app:smoke.test").expect("id cstring");
        let action = CString::new("open").expect("action cstring");
        assert!(look_record_usage(id.as_ptr(), action.as_ptr()));

        let empty = CString::new("").expect("empty cstring");
        assert!(!look_record_usage(empty.as_ptr(), action.as_ptr()));

        let loaded = SqliteStore::open(&db_path)
            .expect("reopen sqlite")
            .load_candidates(None)
            .expect("load candidates after usage");
        let updated = loaded
            .iter()
            .find(|candidate| candidate.id == "app:smoke.test")
            .expect("smoke candidate exists");
        assert_eq!(updated.use_count, 1);
        assert!(updated.last_used_at_unix_s.is_some());

        let _ = fs::remove_file(&db_path);
    }

    fn unique_test_db_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        env::temp_dir().join(format!("look-ffi-smoke-{nanos}.db"))
    }
}
