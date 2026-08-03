//! Motor de rastreo de CrawlForge.
//!
//! Este crate no conoce ninguna UI ni la capa FFI. Se compila y se testea solo.
//! Ver `docs/01-ARQUITECTURA.md`.

// Sonda de asignaciones compartida por los tests de regresión de memoria. Contiene el
// `#[global_allocator]` del binario de tests: solo puede haber uno, así que vive aquí y no
// en el módulo que primero la necesitó (`parse.rs`).
#[cfg(test)]
mod alloc_probe;

pub mod engine;
pub mod entitlement;
pub mod error;
pub mod fetch;
pub mod frontier;
pub mod job;
pub mod normalize;
pub mod parse;
pub mod pattern;
pub mod robots;
pub mod sitemap;
pub mod store;
pub mod throttle;
pub mod writer;

pub use error::CoreError;

/// Version del esquema SQLite que este core escribe.
/// Debe coincidir con la ultima migracion de `migrations/`.
pub const SCHEMA_VERSION: i64 = 7;

pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
