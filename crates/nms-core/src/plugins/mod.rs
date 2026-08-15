//! # Подсистема модулей и плагинов (WASM Plugins)
//!
//! Обеспечивает модульную расширяемость платформы:
//! - [`loader`]: Zero-Unpack загрузка плагинов из архивов `.nms-plugin` / ZIP, упаковщик пакетов и верификация Ed25519 ([`PluginPackage`]).
//! - [`manager`]: Центральный менеджер регистрации, топологической загрузки по DAG и изоляции конфигураций ([`PluginManager`], [`InstalledPlugin`]).

pub mod loader;
pub mod manager;

pub use loader::PluginPackage;
pub use manager::{InstalledPlugin, PluginManager};
