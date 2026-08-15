//! # Системные эндпоинты ядра (/api/v1/system)

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use nms_common::error::ErrorResponse;
use nms_common::i18n::{global, Locale};
use nms_core::services::AuditLogRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/info", get(system_info_handler))
        .route("/i18n/{locale}", get(i18n_export_handler))
        .route("/audit", get(audit_logs_handler))
}

#[derive(Debug, Serialize)]
pub struct SystemInfoResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
    pub dev_mode: bool,
    pub safe_mode: bool,
}

/// GET /api/v1/system/info
async fn system_info_handler(State(state): State<AppState>) -> Json<SystemInfoResponse> {
    let uptime = state.start_time.elapsed().as_secs();
    Json(SystemInfoResponse {
        name: "NMSNext-Gen Universal Core",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
        dev_mode: state.config.server.dev_mode,
        safe_mode: state.config.server.safe_mode,
    })
}

/// GET /api/v1/system/i18n/:locale
async fn i18n_export_handler(Path(locale_str): Path<String>) -> Json<HashMap<String, String>> {
    let locale = Locale::from_str_relaxed(&locale_str);
    let dict = global().export_locale(locale);
    Json(dict)
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub limit: Option<u32>,
    pub after_id: Option<i64>,
}

/// GET /api/v1/system/audit
async fn audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    _auth: AuthUser,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<AuditLogRecord>>, (StatusCode, Json<ErrorResponse>)> {
    let limit = query.limit.unwrap_or(50);
    let logs = state
        .audit_service
        .list_logs(limit, query.after_id)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(logs))
}
