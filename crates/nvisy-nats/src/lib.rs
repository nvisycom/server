#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Task-focused NATS client for the Nvisy platform.
//!
//! This crate provides a minimal, task-focused NATS client with specialized modules for:
//! - **Client**: Connection management and configuration
//! - **KV**: Key-Value store for sessions and caching (NATS KV)
//! - **Stream**: Real-time updates via JetStream for WebSocket broadcasting
//! - **Queue**: Distributed job queues for background processing
//!
//! # Architecture
//!
//! Each module provides focused operations for specific use cases while maintaining
//! access to the underlying NATS client for extensibility.

use std::time::Duration;

// Tracing target constants for consistent logging
pub const TRACING_TARGET_CLIENT: &str = "nvisy_nats::client";
pub const TRACING_TARGET_KV: &str = "nvisy_nats::kv";
pub const TRACING_TARGET_STREAM: &str = "nvisy_nats::stream";
pub const TRACING_TARGET_QUEUE: &str = "nvisy_nats::queue";
pub const TRACING_TARGET_CONNECTION: &str = "nvisy_nats::connection";

pub mod client;
pub mod kv;
pub mod queue;
pub mod stream;

// Re-export key types
pub use async_nats::Error as NatsError;
pub use client::{NatsClient, NatsConfig, NatsConnection, NatsCredentials, NatsTlsConfig};
pub use kv::{CacheStore, DeviceInfo, DeviceType, KvStore, SessionStore, UserSession};
pub use queue::{Job, JobPriority, JobQueue, JobStatus, JobType};
pub use stream::{StreamPublisher, UpdateEvent, UpdateType};

/// Result type for all NATS operations in this crate
pub type Result<T> = std::result::Result<T, Error>;

/// Unified error type for NATS operations
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// NATS client/connection errors
    #[error("NATS connection error: {0}")]
    Connection(#[from] async_nats::Error),

    /// Serialization errors when sending messages
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Operation timeout
    #[error("Operation timed out after {timeout:?}")]
    Timeout { timeout: Duration },

    /// Message delivery failed
    #[error("Message delivery failed to subject '{subject}': {reason}")]
    DeliveryFailed { subject: String, reason: String },

    /// Stream operation failed
    #[error("Stream operation failed on '{stream}': {error}")]
    StreamError { stream: String, error: String },

    /// Job queue operation failed
    #[error("Job queue error on '{queue}': {reason}")]
    JobQueueError { queue: String, reason: String },

    /// Consumer operation failed
    #[error("Consumer '{consumer}' error: {reason}")]
    ConsumerError { consumer: String, reason: String },

    /// Generic operation error with context
    #[error("NATS operation failed: {operation} - {details}")]
    Operation { operation: String, details: String },
}

impl Error {
    /// Check if this error indicates a temporary failure that might succeed on retry
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Connection(_) | Error::Timeout { .. } | Error::DeliveryFailed { .. }
        )
    }

    /// Get the error category for metrics/logging
    pub fn category(&self) -> &'static str {
        match self {
            Error::Connection(_) => "connection",
            Error::Serialization(_) => "serialization",
            Error::Timeout { .. } => "timeout",
            Error::DeliveryFailed { .. } => "delivery",
            Error::StreamError { .. } => "stream",
            Error::JobQueueError { .. } => "job_queue",
            Error::ConsumerError { .. } => "consumer",
            Error::Operation { .. } => "operation",
        }
    }

    /// Create a delivery failed error
    pub fn delivery_failed(subject: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::DeliveryFailed {
            subject: subject.into(),
            reason: reason.into(),
        }
    }

    /// Create a stream error
    pub fn stream_error(stream: impl Into<String>, error: impl Into<String>) -> Self {
        Self::StreamError {
            stream: stream.into(),
            error: error.into(),
        }
    }

    /// Create a job queue error
    pub fn job_queue_error(queue: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::JobQueueError {
            queue: queue.into(),
            reason: reason.into(),
        }
    }

    /// Create a consumer error
    pub fn consumer_error(consumer: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ConsumerError {
            consumer: consumer.into(),
            reason: reason.into(),
        }
    }

    /// Create an operation error with context
    pub fn operation(op: impl Into<String>, details: impl Into<String>) -> Self {
        Self::Operation {
            operation: op.into(),
            details: details.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        let conn_err = Error::Connection(async_nats::Error::new(
            async_nats::ErrorKind::Other,
            Some("test error"),
        ));
        assert_eq!(conn_err.category(), "connection");
        assert!(conn_err.is_retryable());

        let stream_err = Error::stream_error("TEST_STREAM", "Stream not found");
        assert_eq!(stream_err.category(), "stream");
        assert!(!stream_err.is_retryable());
    }
}
