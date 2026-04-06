use crate::runtime_config::{log_debug, log_error};
use crate::state::{cstr_to_string, default_db_path, store_json_allocation};
use look_indexing::{ClipboardContentType, ClipboardItem};
use look_storage::SqliteStore;
use std::ffi::CString;
use std::os::raw::c_char;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(serde::Serialize)]
struct FfiClipboardPayload {
    count: usize,
    items: Vec<FfiClipboardItem>,
    error: Option<FfiClipboardError>,
}

#[derive(serde::Serialize)]
struct FfiClipboardItem {
    id: String,
    content_type: String,
    content: String,
    preview: Option<String>,
    source_app: Option<String>,
    created_at_unix_s: i64,
    last_used_at_unix_s: Option<i64>,
    use_count: u64,
    pinned: bool,
}

#[derive(serde::Serialize)]
struct FfiClipboardError {
    code: &'static str,
    message: String,
}

impl From<&ClipboardItem> for FfiClipboardItem {
    fn from(item: &ClipboardItem) -> Self {
        Self {
            id: item.id.clone(),
            content_type: item.content_type.to_string(),
            content: item.content.clone(),
            preview: item.preview.clone(),
            source_app: item.source_app.clone(),
            created_at_unix_s: item.created_at_unix_s,
            last_used_at_unix_s: item.last_used_at_unix_s,
            use_count: item.use_count,
            pinned: item.pinned,
        }
    }
}

fn error_json(code: &'static str, message: String) -> *mut c_char {
    let payload = FfiClipboardPayload {
        count: 0,
        items: vec![],
        error: Some(FfiClipboardError { code, message }),
    };
    let json = serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"count":0,"items":[],"error":{"code":"serialize_failed","message":""}}"#.to_string()
    });
    let cstring = CString::new(json).expect("valid json");
    store_json_allocation(cstring)
}

pub(crate) fn look_clipboard_store_impl(
    content: *const c_char,
    content_type: *const c_char,
    source_app: *const c_char,
) -> bool {
    let content = cstr_to_string(content);
    let content_type_str = cstr_to_string(content_type);
    let source_app = cstr_to_string(source_app);

    if content.trim().is_empty() {
        return false;
    }

    let ct = match content_type_str.as_str() {
        "text" => ClipboardContentType::Text,
        "image" => ClipboardContentType::Image,
        "file_list" => ClipboardContentType::FileList,
        _ => ClipboardContentType::Text,
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let id = format!("clip:{}:{}", now, &content.get(..32).unwrap_or(&content));

    let preview = match ct {
        ClipboardContentType::Text => {
            let trimmed = content.trim();
            if trimmed.len() > 120 {
                Some(format!("{}...", &trimmed[..120]))
            } else {
                Some(trimmed.to_string())
            }
        }
        ClipboardContentType::Image => Some("[Image]".to_string()),
        ClipboardContentType::FileList => {
            let count = content.lines().count();
            Some(format!("{count} file(s)"))
        }
    };

    let item = ClipboardItem {
        id,
        content_type: ct,
        content,
        preview,
        source_app: if source_app.trim().is_empty() {
            None
        } else {
            Some(source_app)
        },
        created_at_unix_s: now,
        last_used_at_unix_s: None,
        use_count: 0,
        pinned: false,
    };

    let Ok(store) = SqliteStore::open(default_db_path()) else {
        log_error("clipboard_store: failed to open db");
        return false;
    };

    match store.insert_clipboard_item(&item) {
        Ok(()) => {
            log_debug("clipboard_store: item stored");
            true
        }
        Err(err) => {
            log_error(&format!("clipboard_store: {err}"));
            false
        }
    }
}

pub(crate) fn look_clipboard_search_impl(
    query: *const c_char,
    content_type: *const c_char,
    limit: u32,
) -> *mut c_char {
    let query = cstr_to_string(query);
    let content_type_str = cstr_to_string(content_type);
    let max = if limit == 0 { 50 } else { limit as usize };
    let started_at = Instant::now();

    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return error_json("db_open_failed", "Failed to open database".to_string());
    };

    let query_opt = if query.trim().is_empty() {
        None
    } else {
        Some(query.trim())
    };

    let ct_opt = if content_type_str.trim().is_empty() {
        None
    } else {
        Some(content_type_str.trim())
    };

    match store.load_clipboard_items(query_opt, ct_opt, max) {
        Ok(items) => {
            let ffi_items: Vec<FfiClipboardItem> = items.iter().map(FfiClipboardItem::from).collect();
            let count = ffi_items.len();
            let payload = FfiClipboardPayload {
                count,
                items: ffi_items,
                error: None,
            };

            let json = serde_json::to_string(&payload).unwrap_or_else(|_| {
                r#"{"count":0,"items":[],"error":{"code":"serialize_failed","message":""}}"#
                    .to_string()
            });

            log_debug(&format!(
                "clipboard_search query_len={} limit={} count={} elapsed_ms={}",
                query.len(),
                max,
                count,
                started_at.elapsed().as_millis()
            ));

            let cstring = CString::new(json).expect("valid json");
            store_json_allocation(cstring)
        }
        Err(err) => error_json("query_failed", format!("{err}")),
    }
}

pub(crate) fn look_clipboard_delete_impl(item_id: *const c_char) -> bool {
    let item_id = cstr_to_string(item_id);
    if item_id.trim().is_empty() {
        return false;
    }

    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return false;
    };

    store.delete_clipboard_item(item_id.trim()).unwrap_or(false)
}

pub(crate) fn look_clipboard_clear_impl(older_than_seconds: i64) -> u64 {
    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return 0;
    };

    let ts = if older_than_seconds <= 0 {
        None
    } else {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Some(now - older_than_seconds)
    };

    store.clear_clipboard_history(ts).unwrap_or(0)
}

pub(crate) fn look_clipboard_toggle_pin_impl(item_id: *const c_char) -> bool {
    let item_id = cstr_to_string(item_id);
    if item_id.trim().is_empty() {
        return false;
    }

    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return false;
    };

    store.toggle_clipboard_pin(item_id.trim()).unwrap_or(false)
}
