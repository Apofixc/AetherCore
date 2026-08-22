//! # Крейт общих структур, типов и моделей aethercore-common
//!
//! Содержит базовые типы данных, систему ошибок [`AppError`], модели пользователей и RBAC,
//! движок интернационализации [`i18n`], спецификацию и валидатор манифеста плагинов [`manifest`](manifest::ModuleManifest)
//! и конфигурацию платформы [`AppConfig`].

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

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

