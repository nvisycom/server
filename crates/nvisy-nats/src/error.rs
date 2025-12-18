//! Error types and utilities for NATS operations.

use std::time::Duration;

/// Result type for all NATS operations in this crate.
///
/// This is a convenience type alias that defaults to using [`Error`] as the error type.
/// Most functions in this crate return this type for consistent error handling.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Unified error type for NATS operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// NATS client/connection errors
    #[error("NATS connection error: {0}")]
    Connection(#[from] async_nats::Error),

    /// Serialization errors when sending or receiving messages
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

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
            Error::Serialization(_) => "Data format error. Please check your input.".to_string(),
            Error::InvalidConfig { reason } => format!("Configuration error: {}", reason),
            _ => "An unexpected error occurred. Please try again.".to_string(),
        }
    }
}
