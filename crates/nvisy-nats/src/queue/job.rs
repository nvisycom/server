//! Job definitions for background processing.

use std::time::Duration;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Job for background processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub job_type: JobType,
    pub payload: serde_json::Value,
    pub priority: JobPriority,
    pub max_retries: u32,
    pub retry_count: u32,
    pub timeout: Duration,
    pub created_at: Timestamp,
    pub scheduled_for: Option<Timestamp>,
    pub status: JobStatus,
}

impl Job {
    /// Create a new job
    pub fn new(job_type: JobType, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_type,
            payload,
            priority: JobPriority::Normal,
            max_retries: 3,
            retry_count: 0,
            timeout: Duration::from_secs(300), // 5 minutes default
            created_at: Timestamp::now(),
            scheduled_for: None,
            status: JobStatus::Pending,
        }
    }

    /// Set job priority
    pub fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set maximum retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set job timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Schedule job for later execution
    pub fn scheduled_for(mut self, timestamp: Timestamp) -> Self {
        self.scheduled_for = Some(timestamp);
        self
    }

    /// Check if job can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// Check if job is ready to execute (not scheduled for future)
    pub fn is_ready(&self) -> bool {
        self.scheduled_for
            .map(|scheduled| Timestamp::now() >= scheduled)
            .unwrap_or(true)
    }

    /// Get job age
    pub fn age(&self) -> Duration {
        let now = Timestamp::now();
        let signed_dur = now.duration_since(self.created_at);
        Duration::from_secs(signed_dur.as_secs().max(0) as u64)
    }
}

/// Types of jobs that can be processed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    /// Document processing job
    DocumentProcessing,
    /// Image extraction from document
    ImageExtraction,
    /// Text analysis and NLP
    TextAnalysis,
    /// Email notification
    EmailNotification,
    /// Data export
    DataExport,
    /// User cleanup/maintenance
    UserCleanup,
    /// Custom job type
    Custom(String),
}

impl std::fmt::Display for JobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobType::DocumentProcessing => write!(f, "document_processing"),
            JobType::ImageExtraction => write!(f, "image_extraction"),
            JobType::TextAnalysis => write!(f, "text_analysis"),
            JobType::EmailNotification => write!(f, "email_notification"),
            JobType::DataExport => write!(f, "data_export"),
            JobType::UserCleanup => write!(f, "user_cleanup"),
            JobType::Custom(name) => write!(f, "custom_{}", name),
        }
    }
}

/// Job priority levels
#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord
)]
pub enum JobPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl JobPriority {
    /// Get priority as number (for ordering)
    pub fn as_num(&self) -> u8 {
        *self as u8
    }
}

/// Job execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "data")]
pub enum JobStatus {
    /// Job is pending execution
    Pending,

    /// Job is currently running
    Running {
        worker_id: String,
        started_at: Timestamp,
    },

    /// Job completed successfully
    Completed {
        completed_at: Timestamp,
        duration_ms: u64,
        result: Option<serde_json::Value>,
    },

    /// Job failed
    Failed {
        failed_at: Timestamp,
        error: String,
        retry_count: u32,
    },

    /// Job was cancelled
    Cancelled {
        cancelled_at: Timestamp,
        reason: String,
    },
}

impl JobStatus {
    /// Check if job is in terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobStatus::Completed { .. } | JobStatus::Failed { .. } | JobStatus::Cancelled { .. }
        )
    }

    /// Check if job is active
    pub fn is_active(&self) -> bool {
        matches!(self, JobStatus::Running { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_creation() {
        let payload = serde_json::json!({
            "document_id": "123",
            "user_id": "456"
        });

        let job = Job::new(JobType::DocumentProcessing, payload.clone())
            .with_priority(JobPriority::High)
            .with_max_retries(5)
            .with_timeout(Duration::from_secs(600));

        assert_eq!(job.job_type, JobType::DocumentProcessing);
        assert_eq!(job.priority, JobPriority::High);
        assert_eq!(job.max_retries, 5);
        assert_eq!(job.timeout, Duration::from_secs(600));
        assert_eq!(job.payload, payload);
        assert!(job.can_retry());
        assert!(job.is_ready());
    }

    #[test]
    fn test_job_priority_ordering() {
        assert!(JobPriority::Critical > JobPriority::High);
        assert!(JobPriority::High > JobPriority::Normal);
        assert!(JobPriority::Normal > JobPriority::Low);

        assert_eq!(JobPriority::Critical.as_num(), 3);
        assert_eq!(JobPriority::Low.as_num(), 0);
    }

    #[test]
    fn test_job_retry() {
        let mut job =
            Job::new(JobType::EmailNotification, serde_json::json!({})).with_max_retries(2);

        assert!(job.can_retry());
        job.increment_retry();
        assert!(job.can_retry());
        job.increment_retry();
        assert!(!job.can_retry()); // Max retries reached
    }

    #[test]
    fn test_job_status_checks() {
        let pending = JobStatus::Pending;
        assert!(!pending.is_terminal());
        assert!(!pending.is_active());

        let running = JobStatus::Running {
            worker_id: "worker1".to_string(),
            started_at: Timestamp::now(),
        };
        assert!(!running.is_terminal());
        assert!(running.is_active());

        let completed = JobStatus::Completed {
            completed_at: Timestamp::now(),
            duration_ms: 1000,
            result: None,
        };
        assert!(completed.is_terminal());
        assert!(!completed.is_active());
    }

    #[test]
    fn test_job_type_display() {
        assert_eq!(
            JobType::DocumentProcessing.to_string(),
            "document_processing"
        );
        assert_eq!(
            JobType::Custom("backup".to_string()).to_string(),
            "custom_backup"
        );
    }
}
