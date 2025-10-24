#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Task-focused NATS client for the Nvisy platform.
//!
//! This crate provides a minimal, task-focused NATS client with specialized modules for:
//! - **Client**: Connection management and configuration
//! - **KV**: Type-safe Key-Value store for sessions and caching (NATS KV)
//! - **Object**: Object storage for files and binary data (NATS JetStream)
//! - **Stream**: Real-time updates and type-safe job queues via JetStream
//!
//! # Quick Start
//!
//! ```ignore
//! use nvisy_nats::{NatsClient, NatsConfig};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), nvisy_nats::Error> {
//!     // Configure and connect
//!     let config = NatsConfig::new("nats://localhost:4222")
//!         .with_name("my-service")
//!         .with_request_timeout(Duration::from_secs(10));
//!
//!     let client = NatsClient::connect(config).await?;
//!
//!     // Use KV store with type safety
//!     let kv = client.kv_store("my-bucket", None, None).await?;
//!     kv.put("key", &"value").await?;
//!
//!     // Use object storage
//!     let objects = client.object_store("files", None, None).await?;
//!     objects.put_bytes("file.txt", b"content".to_vec().into(), None).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Architecture
//!
//! Each module provides focused, type-safe operations for specific use cases while
//! maintaining access to the underlying NATS client for extensibility. Generic types
//! with PhantomData markers ensure compile-time type safety for payloads and values.

use std::time::Duration;

// Tracing target constants for consistent logging
pub const TRACING_TARGET_CLIENT: &str = "nvisy_nats::client";
pub const TRACING_TARGET_KV: &str = "nvisy_nats::kv";
pub const TRACING_TARGET_OBJECT: &str = "nvisy_nats::object";
pub const TRACING_TARGET_STREAM: &str = "nvisy_nats::stream";
pub const TRACING_TARGET_CONNECTION: &str = "nvisy_nats::connection";

pub mod client;
pub mod kv;
pub mod object;
pub mod retry;
pub mod stream;

// Re-export key types
pub use async_nats::Error as NatsError;
pub use client::{NatsClient, NatsConfig, NatsConnection, NatsCredentials, NatsTlsConfig};
pub use kv::{
    ApiToken, ApiTokenStore, ApiTokenType, CacheStats, CacheStore, KvEntry, KvStore, KvValue,
    TokenStoreStats,
};
pub use object::{GetResult, ObjectInfo, ObjectMeta, ObjectStore, PutResult};
pub use retry::RetryConfig;
pub use stream::{
    DocumentJob, DocumentJobBatchStream, DocumentJobMessage, DocumentJobPayload,
    DocumentJobPriority, DocumentJobPublisher, DocumentJobStatus, DocumentJobStream,
    DocumentJobSubscriber, ProcessingOptions, ProcessingStage, ProcessingType, ProjectEventJob,
    ProjectEventPayload, ProjectEventPriority, ProjectEventPublisher, ProjectEventStatus,
    ProjectExportJob, ProjectExportPayload, ProjectExportPriority, ProjectExportPublisher,
    ProjectExportStatus, ProjectImportJob, ProjectImportPayload, ProjectImportPriority,
    ProjectImportPublisher, ProjectImportStatus, StreamPublisher, StreamSubscriber,
    TypedBatchStream, TypedMessage, TypedMessageStream,
};

/// Result type for all NATS operations in this crate.
///
/// This is a convenience type alias that defaults to using [`Error`] as the error type.
/// Most functions in this crate return this type for consistent error handling.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Check if a NATS server URL is valid.
///
/// This is a simple validation that checks for proper URL format and supported schemes.
///
/// # Examples
///
/// ```
/// use nvisy_nats::is_valid_nats_url;
///
/// assert!(is_valid_nats_url("nats://localhost:4222"));
/// assert!(is_valid_nats_url("tls://secure.nats.io:4222"));
/// assert!(!is_valid_nats_url("http://example.com"));
/// ```
pub fn is_valid_nats_url(url: &str) -> bool {
    url.starts_with("nats://")
        || url.starts_with("tls://")
        || url.starts_with("ws://")
        || url.starts_with("wss://")
}

/// Create a default NATS configuration for development.
///
/// This is equivalent to `NatsConfig::new("nats://localhost:4222")` but with
/// additional development-friendly settings.
///
/// # Examples
///
/// ```ignore
/// use nvisy_nats::dev_config;
///
/// let config = dev_config();
/// // Equivalent to:
/// // let config = NatsConfig::new("nats://localhost:4222")
/// //     .with_name("nvisy-dev");
/// ```
pub fn dev_config() -> NatsConfig {
    NatsConfig::new("nats://localhost:4222").with_name("nvisy-dev")
}

/// Unified error type for NATS operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// NATS client/connection errors
    #[error("NATS connection error: {0}")]
    Connection(#[from] async_nats::Error),

    /// Serialization errors when sending messages
    #[error("Serialization error: {0}")]
    Serialization(serde_json::Error),

    /// Deserialization errors when receiving messages
    #[error("Deserialization error: {0}")]
    Deserialization(serde_json::Error),

    /// JetStream publish error
    #[error("JetStream publish error: {0}")]
    JetstreamPublish(async_nats::error::Error<async_nats::jetstream::context::PublishErrorKind>),

    /// JetStream message error
    #[error("JetStream message error: {0}")]
    JetstreamMessage(Box<dyn std::error::Error + Send + Sync>),

    /// Consumer error
    #[error("Consumer error: {0}")]
    Consumer(String),

    /// Stream error
    #[error("Stream error: {0}")]
    Stream(String),

    /// Acknowledgement error
    #[error("Acknowledgement error: {0}")]
    Ack(String),

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

    /// KV bucket not found
    #[error("KV bucket '{bucket}' not found")]
    KvBucketNotFound { bucket: String },

    /// KV key not found
    #[error("Key '{key}' not found in bucket '{bucket}'")]
    KvKeyNotFound { bucket: String, key: String },

    /// KV revision mismatch (optimistic concurrency failure)
    #[error("Revision mismatch for key '{key}': expected {expected}, got {actual}")]
    KvRevisionMismatch {
        key: String,
        expected: u64,
        actual: u64,
    },

    /// Object store bucket not found
    #[error("Object store bucket '{bucket}' not found")]
    ObjectBucketNotFound { bucket: String },

    /// Object not found in store
    #[error("Object '{name}' not found in bucket '{bucket}'")]
    ObjectNotFound { bucket: String, name: String },

    /// Invalid configuration
    #[error("Invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    /// Generic operation error with context
    #[error("NATS operation failed: {operation} - {details}")]
    Operation { operation: String, details: String },
}

impl Error {
    /// Check if this error indicates a temporary failure that might succeed on retry
    pub fn is_retryable(&self) -> bool {
        match self {
            // Always retryable
            Error::Connection(_) | Error::Timeout { .. } => true,
            // Sometimes retryable - depends on the specific reason
            Error::DeliveryFailed { reason, .. } => {
                // Retry if it's a network/connection issue, but not if it's a validation issue
                reason.contains("connection")
                    || reason.contains("timeout")
                    || reason.contains("network")
            }
            Error::JetstreamPublish(err) => {
                // Retry on temporary JetStream issues
                matches!(
                    err.kind(),
                    async_nats::jetstream::context::PublishErrorKind::Other
                )
            }
            Error::StreamError { error, .. } => {
                // Retry on temporary stream issues
                error.contains("timeout")
                    || error.contains("connection")
                    || error.contains("temporary")
            }
            Error::ConsumerError { reason, .. } => {
                // Retry on temporary consumer issues
                reason.contains("timeout") || reason.contains("connection")
            }
            // Never retryable
            Error::Serialization(_)
            | Error::Deserialization(_)
            | Error::KvRevisionMismatch { .. }
            | Error::KvBucketNotFound { .. }
            | Error::KvKeyNotFound { .. }
            | Error::ObjectBucketNotFound { .. }
            | Error::ObjectNotFound { .. }
            | Error::InvalidConfig { .. } => false,
            // Context-dependent
            _ => false,
        }
    }

    /// Get the error category for metrics/logging
    pub fn category(&self) -> &'static str {
        match self {
            // Connection and network errors
            Error::Connection(_) => "connection",
            Error::Timeout { .. } => "timeout",
            Error::DeliveryFailed { .. } => "delivery",

            // JetStream specific errors
            Error::JetstreamPublish(_) => "jetstream.publish",
            Error::JetstreamMessage(_) => "jetstream.message",
            Error::StreamError { .. } => "jetstream.stream",
            Error::Consumer(_) => "jetstream.consumer",
            Error::ConsumerError { .. } => "jetstream.consumer_error",
            Error::Stream(_) => "jetstream.stream_legacy",
            Error::Ack(_) => "jetstream.ack",

            // Job processing errors
            Error::JobQueueError { .. } => "jobs.queue",

            // Key-Value store errors
            Error::KvBucketNotFound { .. } => "kv.bucket_not_found",
            Error::KvKeyNotFound { .. } => "kv.key_not_found",
            Error::KvRevisionMismatch { .. } => "kv.revision_mismatch",

            // Object store errors
            Error::ObjectBucketNotFound { .. } => "object.bucket_not_found",
            Error::ObjectNotFound { .. } => "object.not_found",

            // Data handling errors
            Error::Serialization(_) => "serialization",
            Error::Deserialization(_) => "deserialization",

            // Configuration errors
            Error::InvalidConfig { .. } => "config",

            // Generic errors
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

    /// Create a KV bucket not found error
    pub fn kv_bucket_not_found(bucket: impl Into<String>) -> Self {
        Self::KvBucketNotFound {
            bucket: bucket.into(),
        }
    }

    /// Create a KV key not found error
    pub fn kv_key_not_found(bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self::KvKeyNotFound {
            bucket: bucket.into(),
            key: key.into(),
        }
    }

    /// Create a KV revision mismatch error
    pub fn kv_revision_mismatch(key: impl Into<String>, expected: u64, actual: u64) -> Self {
        Self::KvRevisionMismatch {
            key: key.into(),
            expected,
            actual,
        }
    }

    /// Create an object bucket not found error
    pub fn object_bucket_not_found(bucket: impl Into<String>) -> Self {
        Self::ObjectBucketNotFound {
            bucket: bucket.into(),
        }
    }

    /// Create an object not found error
    pub fn object_not_found(bucket: impl Into<String>, name: impl Into<String>) -> Self {
        Self::ObjectNotFound {
            bucket: bucket.into(),
            name: name.into(),
        }
    }

    /// Create an invalid configuration error
    pub fn invalid_config(reason: impl Into<String>) -> Self {
        Self::InvalidConfig {
            reason: reason.into(),
        }
    }

    /// Create a timeout error with the given duration
    pub fn timeout(duration: Duration) -> Self {
        Self::Timeout { timeout: duration }
    }

    /// Convert any error to JetstreamMessage error
    pub fn jetstream_message(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::JetstreamMessage(Box::new(error))
    }

    /// Convert boxed error to JetstreamMessage error
    pub fn jetstream_message_boxed(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::JetstreamMessage(error)
    }

    /// Get the error severity level for alerting/monitoring
    pub fn severity(&self) -> &'static str {
        match self {
            // Critical errors that require immediate attention
            Error::InvalidConfig { .. } => "critical",

            // High severity errors that affect functionality
            Error::Connection(_) | Error::StreamError { .. } | Error::JobQueueError { .. } => {
                "high"
            }

            // Medium severity errors that may affect user experience
            Error::Timeout { .. }
            | Error::DeliveryFailed { .. }
            | Error::JetstreamPublish(_)
            | Error::JetstreamMessage(_)
            | Error::Consumer(_)
            | Error::ConsumerError { .. }
            | Error::Stream(_)
            | Error::Ack(_)
            | Error::Operation { .. } => "medium",

            // Low severity errors that are often expected (like not found)
            Error::KvKeyNotFound { .. }
            | Error::ObjectNotFound { .. }
            | Error::KvBucketNotFound { .. }
            | Error::ObjectBucketNotFound { .. } => "low",

            // Client errors that indicate programming issues
            Error::Serialization(_)
            | Error::Deserialization(_)
            | Error::KvRevisionMismatch { .. } => "medium",
        }
    }

    /// Check if this is a client-side error (programming/configuration issue)
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Error::Serialization(_)
                | Error::Deserialization(_)
                | Error::InvalidConfig { .. }
                | Error::KvRevisionMismatch { .. }
        )
    }

    /// Check if this is a "not found" type error
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Error::KvKeyNotFound { .. }
                | Error::KvBucketNotFound { .. }
                | Error::ObjectNotFound { .. }
                | Error::ObjectBucketNotFound { .. }
        )
    }

    /// Check if this is a network-related error
    pub fn is_network_error(&self) -> bool {
        matches!(
            self,
            Error::Connection(_) | Error::Timeout { .. } | Error::DeliveryFailed { .. }
        )
    }

    /// Get a user-friendly error message suitable for display
    pub fn user_message(&self) -> String {
        match self {
            Error::Connection(_) => {
                "Connection to NATS server failed. Please check your connection.".to_string()
            }
            Error::Timeout { timeout } => {
                format!("Operation timed out after {:?}. Please try again.", timeout)
            }
            Error::KvKeyNotFound { key, .. } => format!("Key '{}' not found.", key),
            Error::ObjectNotFound { name, .. } => format!("Object '{}' not found.", name),
            Error::Serialization(_) | Error::Deserialization(_) => {
                "Data format error. Please check your input.".to_string()
            }
            Error::InvalidConfig { reason } => format!("Configuration error: {}", reason),
            _ => "An unexpected error occurred. Please try again.".to_string(),
        }
    }
}

// Manual From implementation for serde_json::Error to default to Serialization
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Serialization(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        // Test with timeout error (doesn't require async_nats::Error construction)
        let timeout_err = Error::Timeout {
            timeout: Duration::from_secs(5),
        };
        assert_eq!(timeout_err.category(), "timeout");
        assert!(timeout_err.is_retryable());
        assert_eq!(timeout_err.severity(), "medium");

        let stream_err = Error::stream_error("TEST_STREAM", "Stream not found");
        assert_eq!(stream_err.category(), "jetstream.stream");
        assert!(!stream_err.is_retryable());
        assert_eq!(stream_err.severity(), "high");

        let delivery_err = Error::delivery_failed("test.subject", "connection failed");
        assert_eq!(delivery_err.category(), "delivery");
        assert!(delivery_err.is_retryable());
        assert_eq!(delivery_err.severity(), "medium");

        let non_retryable_delivery = Error::delivery_failed("test.subject", "invalid payload");
        assert_eq!(non_retryable_delivery.category(), "delivery");
        assert!(!non_retryable_delivery.is_retryable());
    }

    #[test]
    fn test_error_classification() {
        // Create a real serde_json::Error by trying to parse invalid JSON
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let serialization_err = Error::Serialization(json_err);
        assert!(serialization_err.is_client_error());
        assert!(!serialization_err.is_not_found());
        assert!(!serialization_err.is_retryable());

        let kv_not_found = Error::kv_key_not_found("test_bucket", "test_key");
        assert!(kv_not_found.is_not_found());
        assert!(!kv_not_found.is_client_error());
        assert!(!kv_not_found.is_retryable());
        assert_eq!(kv_not_found.severity(), "low");

        let config_err = Error::invalid_config("missing required field");
        assert!(config_err.is_client_error());
        assert!(!config_err.is_retryable());
        assert_eq!(config_err.severity(), "critical");
    }
}
