//! JetStream streams for real-time updates and distributed job processing.
//!
//! This module provides type-safe streaming capabilities for:
//! - Document processing jobs
//! - Project import/export jobs
//! - Project event jobs

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Job execution priority levels
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize
)]
pub enum JobPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Document job execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "data")]
pub enum JobStatus {
    /// Job is queued and waiting to be processed
    Pending,
    /// Job is currently being processed
    Processing {
        started_at: Timestamp,
        worker_id: String,
    },
    /// Job completed successfully
    Completed {
        completed_at: Timestamp,
        duration_ms: u64,
        result: Option<serde_json::Value>,
    },
    /// Job failed with an error
    Failed {
        failed_at: Timestamp,
        error: String,
        retry_scheduled: bool,
    },
    /// Job was cancelled
    Cancelled {
        cancelled_at: Timestamp,
        reason: String,
    },
}

impl JobStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed { .. }
                | JobStatus::Failed {
                    retry_scheduled: false,
                    ..
                }
                | JobStatus::Cancelled { .. }
        )
    }

    pub fn is_processing(&self) -> bool {
        matches!(self, JobStatus::Processing { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, JobStatus::Pending)
    }
}

// Base types
mod publisher;
mod subscriber;

// Document job
mod document_job;
mod document_job_pub;
mod document_job_sub;

// Project import
mod project_import;
mod project_import_pub;
mod project_import_sub;

// Project export
mod project_export;
mod project_export_pub;
mod project_export_sub;

// Project event
mod project_event;
mod project_event_pub;
mod project_event_sub;

// Re-export base types
// Re-export document job types
pub use document_job::{
    DocumentJob, DocumentJobPayload, ProcessingOptions, ProcessingStage, ProcessingType,
};
pub use document_job_pub::DocumentJobPublisher;
pub use document_job_sub::{
    DocumentJobBatchStream, DocumentJobMessage, DocumentJobStream, DocumentJobSubscriber,
};
// Re-export project event types
pub use project_event::{ProjectEventJob, ProjectEventPayload};
pub use project_event_pub::ProjectEventPublisher;
pub use project_event_sub::{
    ProjectEventBatchStream, ProjectEventMessage, ProjectEventStream, ProjectEventSubscriber,
};
// Re-export project export types
pub use project_export::{ProjectExportJob, ProjectExportPayload};
pub use project_export_pub::ProjectExportPublisher;
pub use project_export_sub::{
    ProjectExportBatchStream, ProjectExportMessage, ProjectExportStream, ProjectExportSubscriber,
};
// Re-export project import types
pub use project_import::{ProjectImportJob, ProjectImportPayload};
pub use project_import_pub::ProjectImportPublisher;
pub use project_import_sub::{
    ProjectImportBatchStream, ProjectImportMessage, ProjectImportStream, ProjectImportSubscriber,
};
pub use publisher::StreamPublisher;
pub use subscriber::{StreamSubscriber, TypedBatchStream, TypedMessage, TypedMessageStream};

// Type aliases for backward compatibility
pub type DocumentJobPriority = JobPriority;
pub type DocumentJobStatus = JobStatus;
pub type ProjectEventPriority = JobPriority;
pub type ProjectEventStatus = JobStatus;
pub type ProjectExportPriority = JobPriority;
pub type ProjectExportStatus = JobStatus;
pub type ProjectImportPriority = JobPriority;
pub type ProjectImportStatus = JobStatus;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_priority_ordering() {
        // Test that priorities are ordered correctly
        assert!(JobPriority::Low < JobPriority::Normal);
        assert!(JobPriority::Normal < JobPriority::High);
        assert!(JobPriority::High < JobPriority::Critical);

        // Test numerical values
        assert_eq!(JobPriority::Low as u8, 0);
        assert_eq!(JobPriority::Normal as u8, 1);
        assert_eq!(JobPriority::High as u8, 2);
        assert_eq!(JobPriority::Critical as u8, 3);
    }

    #[test]
    fn test_job_status_pending() {
        let status = JobStatus::Pending;

        assert!(status.is_pending());
        assert!(!status.is_processing());
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_job_status_processing() {
        let status = JobStatus::Processing {
            started_at: Timestamp::now(),
            worker_id: "worker-1".to_string(),
        };

        assert!(!status.is_pending());
        assert!(status.is_processing());
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_job_status_completed() {
        let status = JobStatus::Completed {
            completed_at: Timestamp::now(),
            duration_ms: 5000,
            result: Some(serde_json::json!({"success": true})),
        };

        assert!(!status.is_pending());
        assert!(!status.is_processing());
        assert!(status.is_terminal());
    }

    #[test]
    fn test_job_status_failed_with_retry() {
        let status = JobStatus::Failed {
            failed_at: Timestamp::now(),
            error: "Processing failed".to_string(),
            retry_scheduled: true,
        };

        assert!(!status.is_pending());
        assert!(!status.is_processing());
        assert!(!status.is_terminal()); // Not terminal when retry is scheduled
    }

    #[test]
    fn test_job_status_failed_without_retry() {
        let status = JobStatus::Failed {
            failed_at: Timestamp::now(),
            error: "Processing failed".to_string(),
            retry_scheduled: false,
        };

        assert!(!status.is_pending());
        assert!(!status.is_processing());
        assert!(status.is_terminal()); // Terminal when no retry is scheduled
    }

    #[test]
    fn test_job_status_cancelled() {
        let status = JobStatus::Cancelled {
            cancelled_at: Timestamp::now(),
            reason: "User cancelled".to_string(),
        };

        assert!(!status.is_pending());
        assert!(!status.is_processing());
        assert!(status.is_terminal());
    }

    #[test]
    fn test_type_aliases() {
        // Test that type aliases work correctly
        let doc_priority: DocumentJobPriority = JobPriority::High;
        let event_priority: ProjectEventPriority = JobPriority::Normal;
        let export_priority: ProjectExportPriority = JobPriority::Low;
        let import_priority: ProjectImportPriority = JobPriority::Critical;

        assert_eq!(doc_priority, JobPriority::High);
        assert_eq!(event_priority, JobPriority::Normal);
        assert_eq!(export_priority, JobPriority::Low);
        assert_eq!(import_priority, JobPriority::Critical);

        let doc_status: DocumentJobStatus = JobStatus::Pending;
        let event_status: ProjectEventStatus = JobStatus::Pending;
        let export_status: ProjectExportStatus = JobStatus::Pending;
        let import_status: ProjectImportStatus = JobStatus::Pending;

        assert_eq!(doc_status, JobStatus::Pending);
        assert_eq!(event_status, JobStatus::Pending);
        assert_eq!(export_status, JobStatus::Pending);
        assert_eq!(import_status, JobStatus::Pending);
    }
}
