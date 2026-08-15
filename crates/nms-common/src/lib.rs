//! # Крейт общих структур, типов и моделей nms-common
//!
//! Содержит базовые типы данных, систему ошибок `AppError`, модели пользователей и RBAC,
//! движок интернационализации `i18n`, спецификацию и валидатор манифеста плагинов `manifest.yaml`
//! и конфигурацию платформы.

pub mod config;
pub mod error;
pub mod i18n;
pub mod manifest;
pub mod models;

pub use config::AppConfig;
pub use error::{AppError, ErrorResponse, Result};
pub use i18n::{tr, I18nRegistry, Locale};
pub use manifest::{resolve_module_dag, ModuleCapabilities, ModuleManifest, ModuleType};
pub use models::*;
