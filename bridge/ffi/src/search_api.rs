use crate::runtime_config::log_debug;
use crate::state::{cstr_to_string, store_json_allocation, with_engine};
use std::ffi::CString;
use std::os::raw::c_char;
use std::time::Instant;

#[repr(C)]
pub struct FfiSearchResult {
    pub count: u32,
}

#[derive(serde::Serialize)]
struct FfiSearchPayload {
    query: String,
    count: usize,
    results: Vec<look_engine::LaunchResult>,
    error: Option<FfiErrorPayload>,
}

#[derive(serde::Serialize)]
struct FfiErrorPayload {
    code: &'static str,
    message: String,
}

pub(crate) fn look_search_count_impl(query_len: u32) -> FfiSearchResult {
    let len = query_len.min(1000);
    let query = "x".repeat(len as usize);
    let results = with_engine(|engine| engine.search(&query, 20));
    FfiSearchResult {
        count: results.len() as u32,
    }
}

pub(crate) fn look_search_json_impl(query: *const c_char, limit: u32) -> *mut c_char {
    let query = cstr_to_string(query);
    let max = if limit == 0 {
        20
    } else {
        limit.min(100) as usize
    };
    let started_at = Instant::now();

    let results = with_engine(|engine| engine.search(&query, max));
    let result_count = results.len();

    let payload = FfiSearchPayload {
        query: query.clone(),
        count: result_count,
        results,
        error: None,
    };

    let json = serde_json::to_string(&payload).unwrap_or_else(|_| {
        serde_json::json!({
            "query": query,
            "count": 0,
            "results": [],
            "error": {
                "code": "serialize_failed",
                "message": "Failed to serialize search results"
            }
        })
        .to_string()
    });
    let cstring = CString::new(json).unwrap_or_else(|_| {
        CString::new("{\"query\":\"\",\"count\":0,\"results\":[]}").expect("valid static json")
    });
    log_debug(&format!(
        "search query_len={} limit={} count={} elapsed_ms={}",
        query.len(),
        max,
        result_count,
        started_at.elapsed().as_millis()
    ));
    store_json_allocation(cstring)
}
