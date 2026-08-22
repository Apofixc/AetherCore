//! # Модели данных платформы
//!
//! Модуль содержит общие модели данных ядра:
//! - [`events`]: структуры сообщений шины и журнала событий ([`EventMessage`], [`ReliableEventRecord`]).
//! - [`user`]: модели учетных записей, RBAC ролей, прав доступа и JWT claims ([`User`], [`Role`], [`Permission`], [`JwtClaims`]).

pub mod events;
pub mod user;

pub use events::*;
pub use user::*;
