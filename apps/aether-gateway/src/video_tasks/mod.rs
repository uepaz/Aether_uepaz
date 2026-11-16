use crate::control::GatewayControlAuthContext;
use crate::GatewayError;
use aether_contracts::{ExecutionPlan, ExecutionTimeouts, ProxySnapshot, RequestBody};

pub(crate) use self::service::VideoTaskService;
pub use self::types::VideoTaskTruthSourceMode;
pub(crate) use self::types::{
    GeminiVideoTaskSeed, LocalVideoTaskContentAction, LocalVideoTaskFollowUpPlan,
    LocalVideoTaskPersistence, LocalVideoTaskReadRefreshPlan, LocalVideoTaskReadResponse,
    LocalVideoTaskRegistryMutation, LocalVideoTaskSeed, LocalVideoTaskSnapshot,
    LocalVideoTaskStatus, LocalVideoTaskSuccessPlan, LocalVideoTaskTransport, OpenAiVideoTaskSeed,
    VideoTaskSyncReportMode,
};
pub(crate) use aether_video_tasks_core::{
    build_internal_finalize_video_plan, build_local_sync_finalize_read_response,
    build_local_sync_finalize_request_path, resolve_local_sync_error_background_report_kind,
    resolve_local_sync_success_background_report_kind,
};

pub(crate) use self::helpers::{
    extract_gemini_short_id_from_cancel_path, extract_gemini_short_id_from_path,
    extract_openai_task_id_from_cancel_path, extract_openai_task_id_from_content_path,
    extract_openai_task_id_from_path, extract_openai_task_id_from_remix_path,
    resolve_video_task_hydration_lookup_key, resolve_video_task_read_lookup_key,
    resolve_video_task_report_lookup, VideoTaskReportLookup,
};

use self::helpers::resolve_local_video_registry_mutation;
use self::store::{FileVideoTaskStore, InMemoryVideoTaskStore, VideoTaskStore};
use self::types::LocalVideoTaskProjectionTarget;

mod helpers;
mod service;
mod store;
mod types;

pub(crate) fn bind_video_follow_up_request_identity(
    follow_up: &mut LocalVideoTaskFollowUpPlan,
    headers: &http::HeaderMap,
) {
    let Some(context) = follow_up
        .report_context
        .as_mut()
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    // The task ID still identifies the original video. This operation has its
    // own candidate and usage lifecycle, keyed by the current execution plan.
    context.insert(
        "request_id".to_string(),
        follow_up.plan.request_id.clone().into(),
    );
    if let Some(trace_id) =
        crate::headers::header_value_str(headers, crate::constants::TRACE_ID_HEADER)
    {
        context.insert("trace_id".to_string(), trace_id.into());
    }
}

pub(crate) fn not_found_body() -> serde_json::Value {
    serde_json::json!({"detail": "Video task not found"})
}

pub(crate) fn not_found_error() -> GatewayError {
    GatewayError::Client {
        status: http::StatusCode::NOT_FOUND,
        message: "Video task not found".to_string(),
    }
}

#[cfg(test)]
mod tests;
