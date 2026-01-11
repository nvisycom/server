#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

/// Tracing target for NATS client operations.
///
/// Use this target for logging client initialization, configuration, and client-level errors.
pub const TRACING_TARGET_CLIENT: &str = "nvisy_nats::client";

/// Tracing target for NATS key-value store operations.
///
/// Use this target for logging KV bucket operations, key operations, and KV-related errors.
pub const TRACING_TARGET_KV: &str = "nvisy_nats::kv";

/// Tracing target for NATS object store operations.
///
/// Use this target for logging object storage operations, bucket operations, and object-related errors.
pub const TRACING_TARGET_OBJECT: &str = "nvisy_nats::object";

/// Tracing target for NATS JetStream operations.
///
/// Use this target for logging stream operations, consumer operations, and JetStream-related errors.
pub const TRACING_TARGET_STREAM: &str = "nvisy_nats::stream";

/// Tracing target for NATS connection operations.
///
/// Use this target for logging connection establishment, reconnection, and connection errors.
pub const TRACING_TARGET_CONNECTION: &str = "nvisy_nats::connection";

mod client;
mod error;
pub mod kv;
pub mod object;
pub mod stream;

// Re-export async_nats types needed by consumers
pub use async_nats::jetstream;
pub use client::{NatsClient, NatsConfig, NatsConnection};
pub use error::{Error, Result};
