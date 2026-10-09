use crate::handlers::shared::{module_available_from_env, read_module_enabled};
use crate::{AppState, GatewayError};
use axum::{
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub(crate) const HEALTH_MONITOR_VISIBILITY_KEY: &str = "module.health_monitor.visibility";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HealthMonitorVisibility {
    pub user_enabled: bool,
    pub admin_enabled: bool,
}

impl Default for HealthMonitorVisibility {
    fn default() -> Self {
        Self {
            user_enabled: true,
            admin_enabled: true,
        }
    }
}

pub(crate) async fn read_health_monitor_visibility(
    state: &AppState,
) -> Result<HealthMonitorVisibility, GatewayError> {
    match state
        .read_system_config_json_value(HEALTH_MONITOR_VISIBILITY_KEY)
        .await?
    {
        None => Ok(HealthMonitorVisibility::default()),
        Some(value) => serde_json::from_value(value)
            .map_err(|_| GatewayError::Internal("Invalid health monitor visibility".into())),
    }
}

pub(crate) async fn health_monitor_active(
    state: &AppState,
    admin: bool,
) -> Result<bool, GatewayError> {
    let available = module_available_from_env("HEALTH_MONITOR_AVAILABLE", true);
    if !read_module_enabled(state, "health_monitor", available).await? {
        return Ok(false);
    }
    let visibility = read_health_monitor_visibility(state).await?;
    Ok(if admin {
        visibility.admin_enabled
    } else {
        visibility.user_enabled
    })
}

// 仅控制监控展示；密钥健康度、熔断恢复与观测采集不经过此检查。
pub(crate) async fn health_monitor_access_response(
    state: &AppState,
    admin: bool,
) -> Option<Response<Body>> {
    match health_monitor_active(state, admin).await {
        Ok(true) => None,
        Ok(false) => Some(
            (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "detail": "健康监控已关闭", "code": "health_monitor_disabled"
                })),
            )
                .into_response(),
        ),
        Err(_) => Some(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "detail": "健康监控状态暂不可用", "code": "health_monitor_unavailable"
                })),
            )
                .into_response(),
        ),
    }
}
