#![allow(unsafe_code)]

mod runtime_config;
mod search_api;
mod seed_api;
mod state;
mod translate_api;
mod usage_api;

use look_engine::QueryEngine;
use search_api::FfiSearchResult;
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn look_search_count(query_len: u32) -> FfiSearchResult {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        search_api::look_search_count_impl(query_len)
    }))
    .unwrap_or(FfiSearchResult { count: 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn look_search_json(query: *const c_char, limit: u32) -> *mut c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        search_api::look_search_json_impl(query, limit)
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn look_search_json_compact(query: *const c_char, limit: u32) -> *mut c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        search_api::look_search_json_compact_impl(query, limit)
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn look_record_usage(candidate_id: *const c_char, action: *const c_char) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        usage_api::look_record_usage_impl(candidate_id, action)
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_record_usage_json(
    candidate_id: *const c_char,
    action: *const c_char,
) -> *mut c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        usage_api::look_record_usage_json_impl(candidate_id, action)
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn look_reload_config() -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Drop the engine's cached `~/.look.config` before anything below reads
        // RuntimeConfig — otherwise the reload would see stale roots/limits.
        look_engine::config::RuntimeConfig::invalidate_cache();
        runtime_config::reload_runtime_config();
        state::restart_index_watchers();
        let path = state::default_db_path();
        if QueryEngine::bootstrap_sqlite(&path).is_err() {
            state::mark_index_dirty();
            return false;
        }
        state::refresh_engine_cache();
        state::clear_index_dirty();
        true
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_seed_uwp_apps_json(json: *const c_char) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        seed_api::look_seed_uwp_apps_json_impl(json)
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_request_index_refresh() -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state::request_background_index_refresh()
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn look_free_cstring(ptr: *mut c_char) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state::free_json_allocation(ptr)
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn look_translate_json(
    text: *const c_char,
    target_lang: *const c_char,
) -> *mut c_char {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        translate_api::look_translate_json_impl(text, target_lang)
    }))
    .unwrap_or(std::ptr::null_mut())
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
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn ffi_search_and_record_usage_smoke() {
        let lock = TEST_MUTEX.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().expect("test lock poisoned");

        let db_path = unique_test_db_path();
        let _ = fs::remove_file(&db_path);

        let mut store = SqliteStore::open(&db_path).expect("open sqlite store");
        store
            .upsert_candidates(&[smoke_candidate()])
            .expect("insert smoke candidate");

        unsafe {
            env::set_var("LOOK_DB_PATH", db_path.as_os_str());
        }
        assert!(look_reload_config());

        let query = CString::new("smoke").expect("query cstring");
        let ptr = look_search_json(query.as_ptr(), 10);
        assert!(!ptr.is_null());

        let raw = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(ptr);

        let payload: serde_json::Value = serde_json::from_str(&raw).expect("valid search payload");

        let mut has_smoke = payload
            .get("results")
            .and_then(|value| value.as_array())
            .is_some_and(|results| {
                results.iter().any(|item| {
                    item.get("id")
                        .and_then(|value| value.as_str())
                        .is_some_and(|id| id == "app:smoke.test")
                })
            });

        if !has_smoke {
            // Background bootstrap refresh can replace the in-memory cache during tests
            // (including racing the cache to empty before the first search). Re-seed the
            // sqlite fixture and refresh the cache before asserting.
            let mut store = SqliteStore::open(&db_path).expect("re-open sqlite store");
            store
                .upsert_candidates(&[smoke_candidate()])
                .expect("reinsert smoke candidate");
            state::refresh_engine_cache();

            let retry_ptr = look_search_json(query.as_ptr(), 10);
            assert!(!retry_ptr.is_null());
            let retry_raw = unsafe { CStr::from_ptr(retry_ptr) }
                .to_string_lossy()
                .into_owned();
            look_free_cstring(retry_ptr);
            let retry_payload: serde_json::Value =
                serde_json::from_str(&retry_raw).expect("valid retry payload");
            has_smoke = retry_payload
                .get("results")
                .and_then(|value| value.as_array())
                .is_some_and(|results| {
                    results.iter().any(|item| {
                        item.get("id")
                            .and_then(|value| value.as_str())
                            .is_some_and(|id| id == "app:smoke.test")
                    })
                });
        }
        assert!(has_smoke);

        let compact_ptr = look_search_json_compact(query.as_ptr(), 10);
        assert!(!compact_ptr.is_null());
        let compact_raw = unsafe { CStr::from_ptr(compact_ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(compact_ptr);
        let compact_payload: serde_json::Value =
            serde_json::from_str(&compact_raw).expect("valid compact payload");
        assert!(compact_payload.get("query").is_none());
        assert!(compact_payload.get("results").is_some());

        let id = CString::new("app:smoke.test").expect("id cstring");
        let action = CString::new("open").expect("action cstring");
        assert!(look_record_usage(id.as_ptr(), action.as_ptr()));

        let usage_ptr = look_record_usage_json(id.as_ptr(), action.as_ptr());
        assert!(!usage_ptr.is_null());
        let usage_raw = unsafe { CStr::from_ptr(usage_ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(usage_ptr);
        let usage_payload: serde_json::Value =
            serde_json::from_str(&usage_raw).expect("valid usage payload");
        assert_eq!(
            usage_payload.get("ok").and_then(|v| v.as_bool()),
            Some(true)
        );

        let empty = CString::new("").expect("empty cstring");
        assert!(!look_record_usage(empty.as_ptr(), action.as_ptr()));
        let invalid_ptr = look_record_usage_json(empty.as_ptr(), action.as_ptr());
        assert!(!invalid_ptr.is_null());
        let invalid_raw = unsafe { CStr::from_ptr(invalid_ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(invalid_ptr);
        let invalid_payload: serde_json::Value =
            serde_json::from_str(&invalid_raw).expect("valid invalid-usage payload");
        assert_eq!(
            invalid_payload.get("ok").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert!(
            invalid_payload
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|v| v.as_str())
                .is_some()
        );

        let bad_action = CString::new("not_a_usage_action").expect("bad action");
        let bad_action_ptr = look_record_usage_json(id.as_ptr(), bad_action.as_ptr());
        assert!(!bad_action_ptr.is_null());
        let bad_action_raw = unsafe { CStr::from_ptr(bad_action_ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(bad_action_ptr);
        let bad_action_payload: serde_json::Value =
            serde_json::from_str(&bad_action_raw).expect("valid bad-action payload");
        assert_eq!(
            bad_action_payload
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|v| v.as_str()),
            Some("invalid_usage_action")
        );

        let loaded = SqliteStore::open(&db_path)
            .expect("reopen sqlite")
            .load_candidates(None)
            .expect("load candidates after usage");
        let updated = loaded
            .iter()
            .find(|candidate| candidate.id.as_ref() == "app:smoke.test")
            .expect("smoke candidate exists");
        assert_eq!(updated.use_count, 2);
        assert!(updated.last_used_at_unix_s.is_some());

        let _ = fs::remove_file(&db_path);
    }

    fn smoke_candidate() -> Candidate {
        Candidate {
            id: "app:smoke.test".into(),
            kind: CandidateKind::App,
            title: "Smoke Test App".into(),
            subtitle: Some("smoke app".into()),
            path: "/Applications/Smoke Test App.app".into(),
            use_count: 0,
            last_used_at_unix_s: None,
        }
    }

    #[test]
    fn ffi_reload_refresh_and_translate_error_smoke() {
        let lock = TEST_MUTEX.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().expect("test lock poisoned");

        let db_path = unique_test_db_path();
        let config_path = unique_test_config_path();
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_file(&config_path);

        fs::write(
            &config_path,
            "lazy_indexing_enabled=true\nfile_scan_roots=\nfile_scan_extra_roots=\napp_scan_roots=\n",
        )
        .expect("write test config");

        unsafe {
            env::set_var("LOOK_DB_PATH", db_path.as_os_str());
            env::set_var("LOOK_CONFIG_PATH", config_path.as_os_str());
        }

        assert!(look_reload_config());

        crate::state::stop_index_watchers_for_test();
        thread::sleep(Duration::from_millis(50));

        crate::state::mark_index_dirty();
        let mut refresh_triggered = false;
        for _ in 0..20 {
            if look_request_index_refresh() {
                refresh_triggered = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            refresh_triggered,
            "expected refresh request to acquire slot at least once"
        );

        thread::sleep(Duration::from_millis(100));

        let text = CString::new("hello").expect("text cstring");
        let bad_lang = CString::new("invalid_lang!").expect("bad lang cstring");
        let bad_lang_ptr = look_translate_json(text.as_ptr(), bad_lang.as_ptr());
        let bad_lang_payload = json_from_ptr(bad_lang_ptr);
        assert_eq!(
            bad_lang_payload
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|v| v.as_str()),
            Some("invalid_target_lang")
        );

        let empty = CString::new("").expect("empty cstring");
        let lang = CString::new("en").expect("lang cstring");
        let empty_ptr = look_translate_json(empty.as_ptr(), lang.as_ptr());
        let empty_payload = json_from_ptr(empty_ptr);
        assert_eq!(
            empty_payload
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|v| v.as_str()),
            Some("empty_text")
        );

        crate::state::stop_index_watchers_for_test();
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_file(&config_path);
    }

    fn json_from_ptr(ptr: *mut std::os::raw::c_char) -> serde_json::Value {
        assert!(!ptr.is_null());
        let raw = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(ptr);
        serde_json::from_str(&raw).expect("valid json payload")
    }

    fn unique_test_db_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        env::temp_dir().join(format!("look-ffi-smoke-{nanos}.db"))
    }

    fn unique_test_config_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        env::temp_dir().join(format!("look-ffi-config-smoke-{nanos}.config"))
    }

    #[test]
    fn ffi_seed_uwp_apps_json_inserts_and_search_finds() {
        let lock = TEST_MUTEX.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().expect("test lock poisoned");

        let db_path = unique_test_db_path();
        let _ = fs::remove_file(&db_path);

        unsafe {
            env::set_var("LOOK_DB_PATH", db_path.as_os_str());
        }
        assert!(look_reload_config());

        // Mirror the JSON format the C# UwpAppService produces (System.Text.Json with
        // [JsonPropertyName] attributes → snake_case keys).
        let json = CString::new(
            r#"[
                {"aumid": "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App", "title": "Terminal"},
                {"aumid": "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App", "title": "Notepad"}
            ]"#,
        )
        .expect("seed json");
        assert!(look_seed_uwp_apps_json(json.as_ptr()));

        // Round-trip via sqlite — make sure the rows actually persisted with the right shape.
        let stored = SqliteStore::open(&db_path)
            .expect("reopen sqlite")
            .load_candidates(None)
            .expect("load candidates");
        let terminal = stored
            .iter()
            .find(|c| c.id.as_ref() == "app:uwp:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App")
            .expect("seeded terminal candidate");
        assert_eq!(terminal.title.as_ref(), "Terminal");
        assert_eq!(
            terminal.path.as_ref(),
            "shell:AppsFolder\\Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"
        );
        assert_eq!(terminal.use_count, 0);
        assert_eq!(terminal.last_used_at_unix_s, None);

        // Search has to surface the seeded entry — without this, the user can't find Terminal
        // via the launcher even though it sits in the DB.
        let query = CString::new("terminal").expect("query");
        let ptr = look_search_json(query.as_ptr(), 10);
        assert!(!ptr.is_null());
        let raw = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        look_free_cstring(ptr);
        let payload: serde_json::Value = serde_json::from_str(&raw).expect("valid search payload");
        let has_terminal = payload
            .get("results")
            .and_then(|value| value.as_array())
            .is_some_and(|results| {
                results.iter().any(|item| {
                    item.get("id")
                        .and_then(|value| value.as_str())
                        .is_some_and(|id| {
                            id == "app:uwp:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"
                        })
                })
            });
        assert!(
            has_terminal,
            "expected seeded UWP Terminal in search results, got: {raw}"
        );

        // Re-seeding must be idempotent and preserve use_count after a launch.
        let id = CString::new("app:uwp:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App").expect("id");
        let action = CString::new("open_app").expect("action");
        assert!(look_record_usage(id.as_ptr(), action.as_ptr()));
        assert!(look_seed_uwp_apps_json(json.as_ptr())); // second seed
        let after = SqliteStore::open(&db_path)
            .expect("reopen")
            .load_candidates(None)
            .expect("load");
        let after_terminal = after
            .iter()
            .find(|c| c.id.as_ref() == "app:uwp:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App")
            .expect("still here");
        assert_eq!(
            after_terminal.use_count, 1,
            "re-seeding must preserve use_count via ON CONFLICT"
        );

        // Re-seed with Notepad omitted — simulates the user uninstalling that UWP app
        // between runs. The vanished row must be pruned so it doesn't keep showing up
        // in search forever (delete_stale_candidates can't reach rows written with
        // indexed_at_unix_s = i64::MAX).
        let json_terminal_only = CString::new(
            r#"[{"aumid": "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App", "title": "Terminal"}]"#,
        )
        .expect("seed json terminal only");
        assert!(look_seed_uwp_apps_json(json_terminal_only.as_ptr()));

        let after_prune = SqliteStore::open(&db_path)
            .expect("reopen for prune check")
            .load_candidates(None)
            .expect("load after prune");
        let ids_after_prune: Vec<&str> = after_prune.iter().map(|c| c.id.as_ref()).collect();
        assert!(ids_after_prune.contains(&"app:uwp:Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"));
        assert!(
            !ids_after_prune.contains(&"app:uwp:Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"),
            "Notepad should have been pruned after disappearing from the seed"
        );

        let _ = fs::remove_file(&db_path);
    }
}
