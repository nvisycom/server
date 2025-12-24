//! Event types for stream processing.
//!
//! This module contains priority levels and status types used across all event streams.

use jiff::Timestamp;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Event execution priority levels.
///
/// Priority determines the order in which events are processed when multiple
/// events are queued. Higher priority events are processed before lower priority ones.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[repr(u8)]
pub enum EventPriority {
    /// Low priority - processed when system resources are available.
    Low = 0,

    /// Normal priority - default for most events.
    #[default]
    Normal = 1,

    /// High priority - processed ahead of normal events.
    High = 2,

    /// Critical priority - processed immediately with highest precedence.
    Critical = 3,
}

impl EventPriority {
    /// Returns the numeric value of the priority level.
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns true if this is a critical priority event.
    #[inline]
    pub const fn is_critical(self) -> bool {
        matches!(self, Self::Critical)
    }

    /// Returns true if this is a high or critical priority event.
    #[inline]
    pub const fn is_high_or_critical(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

/// Event is currently being processed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProcessingStatus {
    /// When processing started.
    pub started_at: Timestamp,

    /// ID of the worker processing this event.
    pub worker_id: String,
}

impl ProcessingStatus {
    /// Create a new processing status.
    pub fn new(worker_id: impl Into<String>) -> Self {
        Self {
            started_at: Timestamp::now(),
            worker_id: worker_id.into(),
        }
    }

    /// Get the duration of processing so far.
    pub fn duration(&self) -> std::time::Duration {
        let now = Timestamp::now();
        let signed_dur = now.duration_since(self.started_at);
        std::time::Duration::from_millis(signed_dur.as_millis().max(0) as u64)
    }
}

/// Event completed successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CompletedStatus {
    /// When the event completed.
    pub completed_at: Timestamp,

    /// How long the event took to process (milliseconds).
    pub duration_ms: u64,

    /// Optional result data from the event processing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

impl CompletedStatus {
    /// Create a new completed status.
    pub fn new(duration_ms: u64) -> Self {
        Self {
            completed_at: Timestamp::now(),
            duration_ms,
            result: None,
        }
    }

    /// Create a completed status with result data.
    pub fn with_result(duration_ms: u64, result: serde_json::Value) -> Self {
        Self {
            completed_at: Timestamp::now(),
            duration_ms,
            result: Some(result),
        }
    }
}

/// Event failed with an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct FailedStatus {
    /// When the event failed.
    pub failed_at: Timestamp,

    /// Error message describing the failure.
    pub error: String,

    /// Whether a retry has been scheduled for this event.
    pub retry_scheduled: bool,
}

impl FailedStatus {
    /// Create a new failed status without retry.
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            failed_at: Timestamp::now(),
            error: error.into(),
            retry_scheduled: false,
        }
    }

    /// Create a new failed status with retry scheduled.
    pub fn with_retry(error: impl Into<String>) -> Self {
        Self {
            failed_at: Timestamp::now(),
            error: error.into(),
            retry_scheduled: true,
        }
    }
}

/// Event was cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CancelledStatus {
    /// When the event was cancelled.
    pub cancelled_at: Timestamp,

    /// Reason for cancellation.
    pub reason: String,
}

impl CancelledStatus {
    /// Create a new cancelled status.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            cancelled_at: Timestamp::now(),
            reason: reason.into(),
        }
    }
}

/// Event execution status.
///
/// Tracks the current state of an event as it progresses through the processing pipeline.
/// Each status variant has an associated struct containing relevant metadata.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "status", content = "data")]
pub enum EventStatus {
    /// Event is queued and waiting to be processed.
    #[default]
    Pending,

    /// Event is currently being processed.
    Processing(ProcessingStatus),

    /// Event completed successfully.
    Completed(CompletedStatus),

    /// Event failed with an error.
    Failed(FailedStatus),

    /// Event was cancelled.
    Cancelled(CancelledStatus),
}

impl EventStatus {
    /// Returns true if this is a terminal status (no further processing will occur).
    ///
    /// Terminal statuses include:
    /// - Completed
    /// - Failed (without retry scheduled)
    /// - Cancelled
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed(_)
                | Self::Failed(FailedStatus {
                    retry_scheduled: false,
                    ..
                })
                | Self::Cancelled(_)
        )
    }

    /// Returns true if the event is currently being processed.
    #[inline]
    pub const fn is_processing(&self) -> bool {
        matches!(self, Self::Processing(_))
    }

    /// Returns true if the event is pending and waiting to be processed.
    #[inline]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Returns true if the event completed successfully.
    #[inline]
    pub const fn is_completed(&self) -> bool {
        matches!(self, Self::Completed(_))
    }

    /// Returns true if the event failed.
    #[inline]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// Returns true if the event was cancelled.
    #[inline]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled(_))
    }

    /// Get a reference to processing status if available.
    pub fn processing_status(&self) -> Option<&ProcessingStatus> {
        match self {
            Self::Processing(status) => Some(status),
            _ => None,
        }
    }

    /// Get a reference to completed status if available.
    pub fn completed_status(&self) -> Option<&CompletedStatus> {
        match self {
            Self::Completed(status) => Some(status),
            _ => None,
        }
    }

    /// Get a reference to failed status if available.
    pub fn failed_status(&self) -> Option<&FailedStatus> {
        match self {
            Self::Failed(status) => Some(status),
            _ => None,
        }
    }

    /// Get a reference to cancelled status if available.
    pub fn cancelled_status(&self) -> Option<&CancelledStatus> {
        match self {
            Self::Cancelled(status) => Some(status),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EventPriority tests
    #[test]
    fn test_priority_ordering() {
        assert!(EventPriority::Low < EventPriority::Normal);
        assert!(EventPriority::Normal < EventPriority::High);
        assert!(EventPriority::High < EventPriority::Critical);
    }

    #[test]
    fn test_priority_numeric_values() {
        assert_eq!(EventPriority::Low.as_u8(), 0);
        assert_eq!(EventPriority::Normal.as_u8(), 1);
        assert_eq!(EventPriority::High.as_u8(), 2);
        assert_eq!(EventPriority::Critical.as_u8(), 3);
    }

    #[test]
    fn test_priority_default() {
        assert_eq!(EventPriority::default(), EventPriority::Normal);
    }

    #[test]
    fn test_priority_checks() {
        assert!(!EventPriority::Low.is_critical());
        assert!(!EventPriority::Normal.is_critical());
        assert!(!EventPriority::High.is_critical());
        assert!(EventPriority::Critical.is_critical());

        assert!(!EventPriority::Low.is_high_or_critical());
        assert!(!EventPriority::Normal.is_high_or_critical());
        assert!(EventPriority::High.is_high_or_critical());
        assert!(EventPriority::Critical.is_high_or_critical());
    }

    #[test]
    fn test_priority_serialization() {
        let priority = EventPriority::High;
        let serialized = serde_json::to_string(&priority).unwrap();
        let deserialized: EventPriority = serde_json::from_str(&serialized).unwrap();
        assert_eq!(priority, deserialized);
    }

    // EventStatus tests
    #[test]
    fn test_pending_status() {
        let status = EventStatus::Pending;

        assert!(status.is_pending());
        assert!(!status.is_processing());
        assert!(!status.is_completed());
        assert!(!status.is_failed());
        assert!(!status.is_cancelled());
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_processing_status() {
        let processing = ProcessingStatus::new("worker-1");
        let status = EventStatus::Processing(processing.clone());

        assert!(!status.is_pending());
        assert!(status.is_processing());
        assert!(!status.is_completed());
        assert!(!status.is_failed());
        assert!(!status.is_cancelled());
        assert!(!status.is_terminal());

        let retrieved = status.processing_status().unwrap();
        assert_eq!(retrieved.worker_id, "worker-1");
    }

    #[test]
    fn test_completed_status() {
        let completed = CompletedStatus::new(5000);
        let status = EventStatus::Completed(completed);

        assert!(!status.is_pending());
        assert!(!status.is_processing());
        assert!(status.is_completed());
        assert!(!status.is_failed());
        assert!(!status.is_cancelled());
        assert!(status.is_terminal());

        let retrieved = status.completed_status().unwrap();
        assert_eq!(retrieved.duration_ms, 5000);
        assert!(retrieved.result.is_none());
    }

    #[test]
    fn test_completed_status_with_result() {
        let result = serde_json::json!({"success": true, "processed": 42});
        let completed = CompletedStatus::with_result(3000, result.clone());
        let status = EventStatus::Completed(completed);

        let retrieved = status.completed_status().unwrap();
        assert_eq!(retrieved.duration_ms, 3000);
        assert_eq!(retrieved.result, Some(result));
    }

    #[test]
    fn test_failed_status_with_retry() {
        let failed = FailedStatus::with_retry("Processing failed");
        let status = EventStatus::Failed(failed);

        assert!(!status.is_pending());
        assert!(!status.is_processing());
        assert!(!status.is_completed());
        assert!(status.is_failed());
        assert!(!status.is_cancelled());
        assert!(!status.is_terminal()); // Not terminal when retry is scheduled

        let retrieved = status.failed_status().unwrap();
        assert_eq!(retrieved.error, "Processing failed");
        assert!(retrieved.retry_scheduled);
    }

    #[test]
    fn test_failed_status_without_retry() {
        let failed = FailedStatus::new("Processing failed");
        let status = EventStatus::Failed(failed);

        assert!(status.is_failed());
        assert!(status.is_terminal()); // Terminal when no retry is scheduled

        let retrieved = status.failed_status().unwrap();
        assert!(!retrieved.retry_scheduled);
    }

    #[test]
    fn test_cancelled_status() {
        let cancelled = CancelledStatus::new("User cancelled");
        let status = EventStatus::Cancelled(cancelled);

        assert!(!status.is_pending());
        assert!(!status.is_processing());
        assert!(!status.is_completed());
        assert!(!status.is_failed());
        assert!(status.is_cancelled());
        assert!(status.is_terminal());

        let retrieved = status.cancelled_status().unwrap();
        assert_eq!(retrieved.reason, "User cancelled");
    }

    #[test]
    fn test_status_default() {
        let status = EventStatus::default();
        assert!(status.is_pending());
    }

    #[test]
    fn test_processing_duration() {
        let processing = ProcessingStatus::new("worker-1");
        let duration = processing.duration();
        assert!(duration.as_millis() < 100); // Should be very small
    }

    #[test]
    fn test_status_serialization() {
        let status = EventStatus::Processing(ProcessingStatus::new("worker-1"));
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: EventStatus = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            EventStatus::Processing(processing) => {
                assert_eq!(processing.worker_id, "worker-1");
            }
            _ => panic!("Expected Processing status"),
        }
    }
}
