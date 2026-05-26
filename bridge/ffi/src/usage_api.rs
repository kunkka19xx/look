use crate::runtime_config::{log_debug, log_error};
use crate::state::{cstr_to_string, default_db_path, store_json_allocation, with_engine_mut};
use look_indexing::{CandidateIdKind, UsageAction};
use look_storage::SqliteStore;
use std::ffi::CString;
use std::os::raw::c_char;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(serde::Serialize)]
struct UsageRecordPayload {
    ok: bool,
    error: Option<FfiErrorPayload>,
}

#[derive(serde::Serialize)]
struct FfiErrorPayload {
    code: &'static str,
    message: String,
}

#[derive(Clone, Copy)]
enum UsageRecordError {
    InvalidInput,
    InvalidCandidateId,
    InvalidUsageAction,
    StorageOpenFailed,
    RecordUsageFailed,
}

impl UsageRecordError {
    fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidCandidateId => "invalid_candidate_id",
            Self::InvalidUsageAction => "invalid_usage_action",
            Self::StorageOpenFailed => "storage_open_failed",
            Self::RecordUsageFailed => "record_usage_failed",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::InvalidInput => "Candidate ID and usage action must be non-empty",
            Self::InvalidCandidateId => "Candidate ID format is invalid for usage recording",
            Self::InvalidUsageAction => "Usage action is invalid for usage recording",
            Self::StorageOpenFailed => "Failed to open backend storage for usage recording",
            Self::RecordUsageFailed => "Failed to persist usage event to backend storage",
        }
    }
}

pub(crate) fn look_record_usage_impl(candidate_id: *const c_char, action: *const c_char) -> bool {
    validate_and_record_usage(candidate_id, action).is_ok()
}

pub(crate) fn look_record_usage_json_impl(
    candidate_id: *const c_char,
    action: *const c_char,
) -> *mut c_char {
    let payload = match validate_and_record_usage(candidate_id, action) {
        Ok(()) => UsageRecordPayload {
            ok: true,
            error: None,
        },
        Err(err) => UsageRecordPayload {
            ok: false,
            error: Some(FfiErrorPayload {
                code: err.code(),
                message: err.message().to_string(),
            }),
        },
    };

    let json = serde_json::to_string(&payload)
        .unwrap_or_else(|_| "{\"ok\":false,\"error\":{\"code\":\"serialize_failed\",\"message\":\"Failed to serialize usage response\"}}".to_string());
    let cstring = CString::new(json).unwrap_or_else(|_| {
        CString::new("{\"ok\":false,\"error\":{\"code\":\"serialize_failed\",\"message\":\"Failed to serialize usage response\"}}")
            .expect("valid static json")
    });
    store_json_allocation(cstring)
}

fn validate_and_record_usage(
    candidate_id: *const c_char,
    action: *const c_char,
) -> Result<(), UsageRecordError> {
    let candidate_id = cstr_to_string(candidate_id);
    let action = cstr_to_string(action);

    if candidate_id.trim().is_empty() || action.trim().is_empty() {
        return Err(UsageRecordError::InvalidInput);
    }

    let trimmed_id = candidate_id.trim();
    let trimmed_action = action.trim();

    if CandidateIdKind::from_candidate_id(trimmed_id).is_none() {
        log_error(&format!(
            "invalid usage record attempt: id={}, action={}",
            trimmed_id, trimmed_action
        ));
        return Err(UsageRecordError::InvalidCandidateId);
    }

    if trimmed_action.parse::<UsageAction>().is_err() {
        return Err(UsageRecordError::InvalidUsageAction);
    }

    let Ok(store) = SqliteStore::open(default_db_path()) else {
        return Err(UsageRecordError::StorageOpenFailed);
    };

    let ok = store.record_usage_event(trimmed_id, trimmed_action).is_ok();

    if ok {
        let used_at_unix_s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        with_engine_mut(|engine| {
            let _ = engine.record_usage_in_memory(trimmed_id, used_at_unix_s);
        });
    }

    if ok {
        log_debug("record_usage success");
    } else {
        log_error("record_usage failed");
        return Err(UsageRecordError::RecordUsageFailed);
    }

    Ok(())
}
