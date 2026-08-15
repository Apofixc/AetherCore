//! # Эндпоинты управления плагинами (/api/v1/modules)

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use nms_common::error::{AppError, ErrorResponse};
use nms_common::manifest::ModuleManifest;
use nms_core::auth::check_permission;
use serde::{Deserialize, Serialize};

/// Создать вложенный роутер управления плагинами `/modules`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_modules_handler))
        .route("/{id}", get(get_module_handler))
        .route("/{id}/enable", post(enable_module_handler))
        .route("/{id}/disable", disable_module_handler_post())
        .route("/{id}/config", get(get_module_config_handler))
        .route("/{id}/config", put(set_module_config_handler))
}

fn disable_module_handler_post() -> axum::routing::MethodRouter<AppState> {
    post(disable_module_handler)
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

/// DTO сводной информации об установленном плагине
#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleSummaryDto {
    /// Идентификатор плагина
    pub id: String,
    /// Отображаемое название плагина
    pub name: String,
    /// Версия плагина
    pub version: String,
    /// Описание функционала
    pub description: String,
    /// Активен ли плагин в рантайме
    pub is_enabled: bool,
    /// Декларативный манифест плагина
    pub manifest: ModuleManifest,
}

/// GET /api/v1/modules
async fn list_modules_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> ApiResult<Vec<ModuleSummaryDto>> {
    check_permission(&claims, "modules.view").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    let plugins = state.plugin_manager.list_plugins();
    let dtos = plugins
        .into_iter()
        .map(|p| ModuleSummaryDto {
            id: p.package.manifest.id.clone(),
            name: p.package.manifest.name.clone(),
            version: p.package.manifest.version.clone(),
            description: p.package.manifest.description.clone(),
            is_enabled: p.is_enabled,
            manifest: p.package.manifest,
        })
        .collect();

    Ok(Json(dtos))
}

/// GET /api/v1/modules/{id}
async fn get_module_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<ModuleSummaryDto> {
    check_permission(&claims, "modules.view").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    let plugin = state
        .plugin_manager
        .get_plugin(&id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(AppError::module_not_found(&id).to_api_response(locale)),
            )
        })?;

    Ok(Json(ModuleSummaryDto {
        id: plugin.package.manifest.id.clone(),
        name: plugin.package.manifest.name.clone(),
        version: plugin.package.manifest.version.clone(),
        description: plugin.package.manifest.description.clone(),
        is_enabled: plugin.is_enabled,
        manifest: plugin.package.manifest,
    }))
}

/// POST /api/v1/modules/{id}/enable
async fn enable_module_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    check_permission(&claims, "modules.manage").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    state
        .plugin_manager
        .enable_plugin(&id)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::BAD_REQUEST),
                Json(e.to_api_response(locale)),
            )
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "modules.enable",
            &format!("modules/{}", id),
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(serde_json::json!({"success": true})))
}

/// POST /api/v1/modules/{id}/disable
async fn disable_module_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    check_permission(&claims, "modules.manage").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    state
        .plugin_manager
        .disable_plugin(&id)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::BAD_REQUEST),
                Json(e.to_api_response(locale)),
            )
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "modules.disable",
            &format!("modules/{}", id),
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(serde_json::json!({"success": true})))
}

/// GET /api/v1/modules/{id}/config
async fn get_module_config_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    check_permission(&claims, "modules.view").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    let config = state
        .plugin_manager
        .get_plugin_config(&id)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(config.unwrap_or(serde_json::json!({}))))
}

/// PUT /api/v1/modules/{id}/config
async fn set_module_config_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(config_val): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    check_permission(&claims, "modules.manage").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    state
        .plugin_manager
        .set_plugin_config(&id, &config_val)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::BAD_REQUEST),
                Json(e.to_api_response(locale)),
            )
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "modules.config_update",
            &format!("modules/{}/config", id),
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(serde_json::json!({"success": true})))
}
