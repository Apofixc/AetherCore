//! # Подсистема модулей и плагинов (WASM Plugins)

pub mod loader;
pub mod manager;

pub use loader::PluginPackage;
pub use manager::{InstalledPlugin, PluginManager};
