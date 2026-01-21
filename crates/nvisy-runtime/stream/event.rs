//! Event types for stream processing.
//!
//! This module contains common event types and the file job type
//! used in processing pipelines.

use jiff::Timestamp;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// File processing job.
///
/// Represents a unit of work in a file processing pipeline.
/// Each job targets a specific file and carries a generic payload
/// that defines the processing parameters.
///
/// The generic parameter `T` is the job-specific data payload.
/// Callers define their own payload types for different pipeline stages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct FileJob<T> {
    /// Unique job identifier (UUID v7 for time-ordering).
    pub id: Uuid,
    /// Database file ID to process.
    pub file_id: Uuid,
    /// Storage path in NATS object store (DocumentKey encoded).
    pub object_key: String,
    /// File extension for format detection.
    pub file_extension: String,
    /// Job-specific data payload.
    pub data: T,
    /// When the job was created.
    pub created_at: Timestamp,
    /// NATS subject to publish result to (for internal job chaining).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback_subject: Option<String>,
    /// Idempotency key to prevent duplicate job processing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl<T> FileJob<T> {
    /// Creates a new file job with the given data payload.
    pub fn new(file_id: Uuid, object_key: String, file_extension: String, data: T) -> Self {
        Self {
            id: Uuid::now_v7(),
            file_id,
            object_key,
            file_extension,
            data,
            created_at: Timestamp::now(),
            callback_subject: None,
            idempotency_key: None,
        }
    }

    /// Sets a callback subject for job chaining.
    pub fn with_callback(mut self, subject: impl Into<String>) -> Self {
        self.callback_subject = Some(subject.into());
        self
    }

    /// Sets an idempotency key.
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Returns job age since creation.
    pub fn age(&self) -> std::time::Duration {
        let now = Timestamp::now();
        let signed_dur = now.duration_since(self.created_at);
        std::time::Duration::from_secs(signed_dur.as_secs().max(0) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestPayload {
        message: String,
    }

    #[test]
    fn test_serialization_roundtrip() {
        let file_id = Uuid::now_v7();
        let job = FileJob::new(
            file_id,
            "path".to_string(),
            "pdf".to_string(),
            TestPayload {
                message: "hello".to_string(),
            },
        );

        let json = serde_json::to_string(&job).unwrap();
        let parsed: FileJob<TestPayload> = serde_json::from_str(&json).unwrap();

        assert_eq!(job.file_id, parsed.file_id);
        assert_eq!(job.data, parsed.data);
    }

    #[test]
    fn test_with_unit_payload() {
        let file_id = Uuid::now_v7();
        let job: FileJob<()> = FileJob::new(file_id, "path".to_string(), "pdf".to_string(), ());

        let json = serde_json::to_string(&job).unwrap();
        let parsed: FileJob<()> = serde_json::from_str(&json).unwrap();

        assert_eq!(job.file_id, parsed.file_id);
    }
}
