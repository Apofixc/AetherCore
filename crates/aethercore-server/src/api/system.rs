//! # Системные эндпоинты ядра (`/api/v1/system`)
//!
//! Предоставляет HTTP эндпоинты для:
//! - Получения статуса и версии ядра (`GET /api/v1/system/info`).
//! - Экспорта словарей локализации (`GET /api/v1/system/i18n/{locale}`).
//! - Чтения журнала аудита (`GET /api/v1/system/audit`).
//! - Получения списка лог-провайдеров (`GET /api/v1/system/logs/providers`).
//! - Поиска и фильтрации системных логов (`GET /api/v1/system/logs`).
//! - Скачивания файла системного журнала (`GET /api/v1/system/logs/download`).

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use aethercore_common::error::{AppError, ErrorResponse};
use aethercore_common::i18n::{global, Locale};
use aethercore_core::auth::check_permission;
use aethercore_core::services::{AuditArchiveInfo, AuditLogRecord, LogLevel, LogProvider, LogQueryResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Создать вложенный роутер системных эндпоинтов `/system`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/info", get(system_info_handler))
        .route("/i18n/{locale}", get(i18n_export_handler))
        .route("/audit", get(audit_logs_handler).delete(clear_audit_logs_handler))
        .route("/audit/count", get(audit_count_handler))
        .route("/audit/rotate", post(rotate_audit_logs_handler))
        .route("/audit/import", post(import_audit_logs_handler))
        .route("/audit/archives", get(list_audit_archives_handler))
        .route("/audit/archives/{filename}", get(download_audit_archive_handler))
        .route("/logs/providers", get(log_providers_handler))
        .route("/logs", get(logs_query_handler))
        .route("/logs/download", get(logs_download_handler))
}

/// Ответ REST API с метаинформацией о запущенном экземпляре ядра платформы
#[derive(Debug, Serialize)]
pub struct SystemInfoResponse {
    /// Имя платформы
    pub name: &'static str,
    /// Версия платформы
    pub version: &'static str,
    /// Время непрерывной работы в секундах
    pub uptime_seconds: u64,
    /// Флаг режима разработки (`dev_mode`)
    pub dev_mode: bool,
    /// Флаг аварийного безопасного режима (`safe_mode`)
    pub safe_mode: bool,
}

/// GET /api/v1/system/info
///
/// Получить информацию о версии ядра, времени непрерывной работы (uptime) и активных режимах работы (`dev_mode`, `safe_mode`).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
///
/// # Возвращаемое значение
/// Структура [`SystemInfoResponse`].
async fn system_info_handler(State(state): State<AppState>) -> Json<SystemInfoResponse> {
    let uptime = state.start_time.elapsed().as_secs();
    Json(SystemInfoResponse {
        name: "AetherCore Platform",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
        dev_mode: state.config.server.dev_mode,
        safe_mode: state.config.server.safe_mode,
    })
}

/// GET /api/v1/system/i18n/:locale
///
/// Экспортировать полный словарь локализации для указанного языка (например, `"ru"` или `"en"`).
///
/// # Аргументы
/// * `locale_str` — Строковый код языка (`"ru"`, `"en"`).
///
/// # Возвращаемое значение
/// JSON-словарь пар `ключ -> шаблон_перевода`.
async fn i18n_export_handler(Path(locale_str): Path<String>) -> Json<HashMap<String, String>> {
    let locale = Locale::from_str_relaxed(&locale_str);
    let dict = global().export_locale(locale);
    Json(dict)
}

/// Параметры постраничной пагинации и поиска журнала аудита
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    /// Максимальное число возвращаемых записей (по умолчанию 50, макс 500)
    pub limit: Option<u32>,
    /// Смещение для постраничной пагинации
    pub offset: Option<u64>,
    /// Поисковый запрос
    pub search: Option<String>,
    /// Идентификатор последнего прочитанного события (для обратной совместимости)
    pub after_id: Option<i64>,
}

/// Ответ REST API с общим количеством записей журнала аудита
#[derive(Debug, Serialize)]
pub struct AuditCountResponse {
    /// Общее количество записей
    pub total: i64,
}

/// GET /api/v1/system/audit/count
///
/// Получить общее количество записей журнала аудита безопасности с учетом фильтра.
async fn audit_count_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditCountResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let total = state
        .audit_service
        .count_logs(query.search.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(AuditCountResponse { total }))
}

/// GET /api/v1/system/audit
///
/// Получить записи журнала аудита безопасности и действий пользователей.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `_auth` — Авторизованный пользователь [`AuthUser`].
/// * `query` — Параметры пагинации [`AuditQuery`].
///
/// # Возвращаемое значение
/// Список записей аудита [`AuditLogRecord`] и заголовок `X-Total-Count`.
async fn audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(query): Query<AuditQuery>,
) -> Result<(HeaderMap, Json<Vec<AuditLogRecord>>), (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let limit = query.limit.unwrap_or(50);
    let total = state
        .audit_service
        .count_logs(query.search.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let logs = state
        .audit_service
        .list_logs(limit, query.offset, query.search.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-total-count",
        total.to_string().parse().unwrap_or(axum::http::HeaderValue::from_static("0")),
    );

    Ok((headers, Json(logs)))
}

/// Ответ REST API на очистку журнала аудита
#[derive(Debug, Serialize)]
pub struct ClearAuditResponse {
    /// Флаг успешного выполнения
    pub success: bool,
    /// Количество удаленных записей
    pub deleted_count: u64,
}

/// DELETE /api/v1/system/audit
///
/// Очистить журнал аудита безопасности (доступно суперпользователю или с правами access.manage).
async fn clear_audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    headers: HeaderMap,
    AuthUser(claims): AuthUser,
) -> Result<Json<ClearAuditResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let client_ip = crate::middleware::extract_client_ip(&headers);

    let deleted_count = state
        .audit_service
        .clear_logs()
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    // Фиксируем факт очистки журнала аудита
    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "audit.clear",
            "system/audit",
            "success",
            Some(&format!("Cleared {} audit records", deleted_count)),
            Some(&client_ip),
        )
        .await;

    Ok(Json(ClearAuditResponse {
        success: true,
        deleted_count,
    }))
}

fn default_true() -> bool {
    true
}

/// Параметры запроса на ротацию журнала аудита
#[derive(Debug, Deserialize)]
pub struct RotateAuditRequest {
    /// Срок давности в днях (по умолчанию 90)
    pub days: Option<u32>,
    /// Сохранить ли архивный JSON-файл перед удалением
    #[serde(default = "default_true")]
    pub archive: bool,
}

/// Ответ на запрос ротации журнала аудита
#[derive(Debug, Serialize)]
pub struct RotateAuditResponse {
    /// Флаг успеха
    pub success: bool,
    /// Количество удаленных записей
    pub deleted_count: u64,
    /// Имя созданного архивного файла
    pub archive_filename: Option<String>,
}

/// POST /api/v1/system/audit/rotate
///
/// Выполнить ротацию журнала аудита (удаление записей старше N дней с опциональной архивацией).
async fn rotate_audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    headers: HeaderMap,
    AuthUser(claims): AuthUser,
    Json(payload): Json<RotateAuditRequest>,
) -> Result<Json<RotateAuditResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let client_ip = crate::middleware::extract_client_ip(&headers);
    let retention_days = payload.days.unwrap_or(90);
    let archive_dir = std::path::PathBuf::from("data/archives");

    let (deleted_count, archive_filename) = state
        .audit_service
        .archive_and_prune(retention_days, payload.archive, &archive_dir)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let details = format!(
        "Rotated audit logs older than {} days (deleted: {}, archive: {:?})",
        retention_days, deleted_count, archive_filename
    );

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "audit.rotate",
            "system/audit",
            "success",
            Some(&details),
            Some(&client_ip),
        )
        .await;

    Ok(Json(RotateAuditResponse {
        success: true,
        deleted_count,
        archive_filename,
    }))
}

/// Запрос на импорт записей аудита из архива
#[derive(Debug, Deserialize)]
pub struct ImportAuditRequest {
    /// Список записей аудита для вставки
    pub records: Vec<AuditLogRecord>,
}

/// Ответ на запрос импорта записей аудита
#[derive(Debug, Serialize)]
pub struct ImportAuditResponse {
    /// Флаг успеха
    pub success: bool,
    /// Количество успешно импортированных записей
    pub imported_count: usize,
}

/// POST /api/v1/system/audit/import
///
/// Импортировать записи аудита из файла архива в базу данных.
async fn import_audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    headers: HeaderMap,
    AuthUser(claims): AuthUser,
    Json(payload): Json<ImportAuditRequest>,
) -> Result<Json<ImportAuditResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let client_ip = crate::middleware::extract_client_ip(&headers);

    let imported_count = state
        .audit_service
        .import_logs(&payload.records)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let details = format!("Imported {} audit records from archive", imported_count);

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "audit.import",
            "system/audit",
            "success",
            Some(&details),
            Some(&client_ip),
        )
        .await;

    Ok(Json(ImportAuditResponse {
        success: true,
        imported_count,
    }))
}

/// GET /api/v1/system/audit/archives
///
/// Получить список всех сохраненных файлов архива журнала аудита.
async fn list_audit_archives_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<AuditArchiveInfo>>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let archive_dir = std::path::PathBuf::from("data/archives");
    let archives = state
        .audit_service
        .list_archives(&archive_dir)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(archives))
}

/// GET /api/v1/system/audit/archives/{filename}
///
/// Скачать архивный JSON-файл журнала аудита.
async fn download_audit_archive_handler(
    Path(filename): Path<String>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        let err = AppError::bad_request("Invalid filename");
        return Err((StatusCode::BAD_REQUEST, Json(err.to_api_response(locale))));
    }

    let archive_path = std::path::PathBuf::from("data/archives").join(&filename);
    if !archive_path.exists() {
        let err = AppError::not_found(format!("Archive '{}' not found", filename));
        return Err((StatusCode::NOT_FOUND, Json(err.to_api_response(locale))));
    }

    let content = tokio::fs::read(&archive_path).await.map_err(|e| {
        let err = AppError::internal(e.to_string());
        (StatusCode::INTERNAL_SERVER_ERROR, Json(err.to_api_response(locale)))
    })?;

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    headers.insert(
        CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename).parse().unwrap(),
    );

    Ok((headers, content))
}

/// GET /api/v1/system/logs/providers
///
/// Получить список доступных источников логов (ядро, модули, системные файлы).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `_auth` — Авторизованный пользователь [`AuthUser`].
///
/// # Возвращаемое значение
/// Список доступных провайдеров логов [`LogProvider`].
async fn log_providers_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<LogProvider>>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let providers = state.logger_service.list_providers().map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(providers))
}

/// Параметры фильтрации и поиска системных логов
#[derive(Debug, Deserialize)]
pub struct LogQueryParams {
    /// Идентификатор провайдера логов (по умолчанию `"system"`)
    pub provider: Option<String>,
    /// Максимальное число возвращаемых строк
    pub limit: Option<usize>,
    /// Минимальный уровень логирования для фильтрации (`TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`)
    pub level: Option<String>,
    /// Подстрока полнотекстового поиска по содержимому сообщения
    pub search: Option<String>,
}

/// GET /api/v1/system/logs
///
/// Выполнить поиск и выборку записей логов по источнику, уровню и ключевым словам.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `_auth` — Авторизованный пользователь [`AuthUser`].
/// * `params` — Параметры поиска [`LogQueryParams`].
///
/// # Возвращаемое значение
/// Результаты поиска логов [`LogQueryResult`].
async fn logs_query_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(params): Query<LogQueryParams>,
) -> Result<Json<LogQueryResult>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

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
    /// Идентификатор источника логов (по умолчанию `"system"`)
    pub provider: Option<String>,
}

/// GET /api/v1/system/logs/download
///
/// Скачать полный файл логов указанного провайдера в виде бинарного потока `text/plain` с заголовком `Content-Disposition: attachment`.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `_auth` — Авторизованный пользователь [`AuthUser`].
/// * `params` — Параметры скачивания [`LogDownloadParams`].
async fn logs_download_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(params): Query<LogDownloadParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }
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
