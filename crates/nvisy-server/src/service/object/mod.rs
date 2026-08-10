//! Object-store access.
//!
//! Bridges stored workspace connections to the [`nvisy_object`] providers: a
//! connection carries an encrypted, typed [`StorageConfig`](nvisy_object::providers::StorageConfig),
//! which [`ObjectService`] turns into a connected client at runtime. The sync
//! orchestration built on top lives in the [`sync`](crate::service::sync) module.

mod service;

pub use service::ObjectService;
