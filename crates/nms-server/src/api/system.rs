//! # Системные эндпоинты ядра (/api/v1/system)

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use nms_common::error::ErrorResponse;
use nms_common::i18n::{global, Locale};
use nms_core::services::{AuditLogRecord, LogLevel, LogProvider, LogQueryResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Создать вложенный роутер системных эндпоинтов `/system`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/info", get(system_info_handler))
        .route("/i18n/{locale}", get(i18n_export_handler))
        .route("/audit", get(audit_logs_handler))
        .route("/logs/providers", get(log_providers_handler))
        .route("/logs", get(logs_query_handler))
        .route("/logs/download", get(logs_download_handler))
}

/// Ответ с метаинформацией о запущенном экземпляре ядра
#[derive(Debug, Serialize)]
pub struct SystemInfoResponse {
    /// Имя платформы
    pub name: &'static str,
    /// Версия платформы
    pub version: &'static str,
    /// Время непрерывной работы в секундах
    pub uptime_seconds: u64,
    /// Флаг режима разработки
    pub dev_mode: bool,
    /// Флаг безопасного режима
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

/// Параметры запроса журнала аудита
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    /// Лимит возвращаемых записей
    pub limit: Option<u32>,
    /// ID последней прочитанной записи для пагинации
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

/// GET /api/v1/system/logs/providers
async fn log_providers_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    _auth: AuthUser,
) -> Result<Json<Vec<LogProvider>>, (StatusCode, Json<ErrorResponse>)> {
    let providers = state.logger_service.list_providers().map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(providers))
}

/// Параметры выборки системных логов
#[derive(Debug, Deserialize)]
pub struct LogQueryParams {
    /// Идентификатор провайдера логов (по умолчанию `"system"`)
    pub provider: Option<String>,
    /// Максимальное число возвращаемых строк
    pub limit: Option<usize>,
    /// Минимальный уровень логирования для фильтрации
    pub level: Option<String>,
    /// Подстрока полнотекстового поиска
    pub search: Option<String>,
}

/// GET /api/v1/system/logs
async fn logs_query_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    _auth: AuthUser,
    Query(params): Query<LogQueryParams>,
) -> Result<Json<LogQueryResult>, (StatusCode, Json<ErrorResponse>)> {
    let provider_id = params.provider.as_deref().unwrap_or("system");
    let limit = params.limit.unwrap_or(200);
    let min_level = params.level.as_deref().and_then(LogLevel::from_str_loose);

    let result = state
        .logger_service
        .get_logs(provider_id, limit, min_level, params.search.as_deref())
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(result))
}

/// Параметры скачивания файла логов
#[derive(Debug, Deserialize)]
pub struct LogDownloadParams {
    /// Идентификатор источника логов
    pub provider: Option<String>,
}

/// GET /api/v1/system/logs/download
async fn logs_download_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    _auth: AuthUser,
    Query(params): Query<LogDownloadParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let provider_id = params.provider.as_deref().unwrap_or("system");

    let (bytes, filename) = state
        .logger_service
        .download_log(provider_id)
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "text/plain; charset=utf-8".parse().unwrap());
    headers.insert(
        CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    Ok((headers, bytes))
}
