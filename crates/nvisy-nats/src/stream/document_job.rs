//! Document job types and shared structures.

use std::time::Duration;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::event::{EventPriority, EventStatus};

/// Default maximum number of retries for a job
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default timeout for job processing in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// Document processing stages
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStage {
    /// Initial document preprocessing
    Preprocessing,
    /// Main document/file processing
    #[default]
    Processing,
    /// Final document postprocessing
    Postprocessing,
}

/// Document processing type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingType {
    /// OCR processing
    Ocr,
    /// Text extraction
    TextExtraction,
    /// Format conversion
    FormatConversion,
    /// Content analysis
    ContentAnalysis,
}

/// Document processing options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessingOptions {
    /// Quality settings
    pub quality: Option<String>,
    /// Language settings
    pub language: Option<String>,
    /// Custom parameters
    pub custom_params: Option<serde_json::Value>,
}

/// Document job payload containing processing details
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentJobPayload {
    pub document_id: Uuid,
    pub account_id: Uuid,
    pub stage: ProcessingStage,

    // File processing fields
    pub file_id: Option<Uuid>,
    pub storage_path: Option<String>,
    pub file_extension: Option<String>,
    pub file_size_bytes: Option<i64>,

    // Preprocessing fields
    pub source_format: Option<String>,
    pub target_format: Option<String>,
    pub validation_rules: Option<Vec<String>>,

    // Processing fields
    pub processing_type: Option<ProcessingType>,
    pub options: Option<ProcessingOptions>,

    // Postprocessing fields
    pub cleanup_tasks: Option<Vec<String>>,
    pub finalization_steps: Option<Vec<String>>,
}

impl DocumentJobPayload {
    /// Create a new document job payload with minimal fields
    pub fn new(document_id: Uuid, account_id: Uuid, stage: ProcessingStage) -> Self {
        Self {
            document_id,
            account_id,
            stage,
            file_id: None,
            storage_path: None,
            file_extension: None,
            file_size_bytes: None,
            source_format: None,
            target_format: None,
            validation_rules: None,
            processing_type: None,
            options: None,
            cleanup_tasks: None,
            finalization_steps: None,
        }
    }
}

/// Document processing job
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentJob {
    pub id: Uuid,
    pub payload: DocumentJobPayload,
    pub priority: EventPriority,
    pub max_retries: u32,
    pub retry_count: u32,
    pub timeout: Duration,
    pub created_at: Timestamp,
    pub scheduled_for: Option<Timestamp>,
    pub status: EventStatus,
}

impl DocumentJob {
    /// Create a new file processing job
    pub fn new_file_processing(
        file_id: Uuid,
        document_id: Uuid,
        account_id: Uuid,
        storage_path: String,
        file_extension: String,
        file_size_bytes: i64,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            payload: DocumentJobPayload {
                document_id,
                account_id,
                stage: ProcessingStage::Processing,
                file_id: Some(file_id),
                storage_path: Some(storage_path),
                file_extension: Some(file_extension),
                file_size_bytes: Some(file_size_bytes),
                source_format: None,
                target_format: None,
                validation_rules: None,
                processing_type: None,
                options: None,
                cleanup_tasks: None,
                finalization_steps: None,
            },
            priority: EventPriority::Normal,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_count: 0,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            created_at: Timestamp::now(),
            scheduled_for: None,
            status: EventStatus::Pending,
        }
    }

    /// Create a new preprocessing job
    pub fn new_preprocessing(
        document_id: Uuid,
        account_id: Uuid,
        source_format: String,
        target_format: Option<String>,
        validation_rules: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            payload: DocumentJobPayload {
                document_id,
                account_id,
                stage: ProcessingStage::Preprocessing,
                file_id: None,
                storage_path: None,
                file_extension: None,
                file_size_bytes: None,
                source_format: Some(source_format),
                target_format,
                validation_rules: Some(validation_rules),
                processing_type: None,
                options: None,
                cleanup_tasks: None,
                finalization_steps: None,
            },
            priority: EventPriority::Normal,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_count: 0,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            created_at: Timestamp::now(),
            scheduled_for: None,
            status: EventStatus::Pending,
        }
    }

    /// Create a new postprocessing job
    pub fn new_postprocessing(
        document_id: Uuid,
        account_id: Uuid,
        cleanup_tasks: Vec<String>,
        finalization_steps: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            payload: DocumentJobPayload {
                document_id,
                account_id,
                stage: ProcessingStage::Postprocessing,
                file_id: None,
                storage_path: None,
                file_extension: None,
                file_size_bytes: None,
                source_format: None,
                target_format: None,
                validation_rules: None,
                processing_type: None,
                options: None,
                cleanup_tasks: Some(cleanup_tasks),
                finalization_steps: Some(finalization_steps),
            },
            priority: EventPriority::Normal,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_count: 0,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            created_at: Timestamp::now(),
            scheduled_for: None,
            status: EventStatus::Pending,
        }
    }

    /// Set job priority
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Schedule job for later execution
    pub fn scheduled_for(mut self, when: Timestamp) -> Self {
        self.scheduled_for = Some(when);
        self
    }

    /// Check if job can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries && !self.status.is_terminal()
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// Get the file ID (if this is a file processing job)
    pub fn file_id(&self) -> Option<Uuid> {
        self.payload.file_id
    }

    /// Get the document ID
    pub fn document_id(&self) -> Uuid {
        self.payload.document_id
    }

    /// Get the account ID
    pub fn account_id(&self) -> Uuid {
        self.payload.account_id
    }

    /// Check if job is ready to execute
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocessing_job_creation() {
        let job = DocumentJob::new_preprocessing(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "pdf".to_string(),
            Some("docx".to_string()),
            vec!["validate_structure".to_string()],
        );

        assert_eq!(job.payload.stage, ProcessingStage::Preprocessing);
        assert_eq!(job.priority, EventPriority::Normal);
        assert_eq!(job.status, EventStatus::Pending);
        assert!(job.can_retry());
        assert_eq!(job.payload.source_format, Some("pdf".to_string()));
        assert_eq!(job.payload.target_format, Some("docx".to_string()));
        assert_eq!(
            job.payload.validation_rules,
            Some(vec!["validate_structure".to_string()])
        );
    }

    #[test]
    fn test_postprocessing_job_creation() {
        let cleanup_tasks = vec!["cleanup_temp_files".to_string()];
        let finalization_steps = vec!["update_metadata".to_string()];

        let job = DocumentJob::new_postprocessing(
            Uuid::new_v4(),
            Uuid::new_v4(),
            cleanup_tasks.clone(),
            finalization_steps.clone(),
        );

        assert_eq!(job.payload.stage, ProcessingStage::Postprocessing);
        assert_eq!(job.payload.cleanup_tasks, Some(cleanup_tasks));
        assert_eq!(job.payload.finalization_steps, Some(finalization_steps));
        assert_eq!(job.priority, EventPriority::Normal);
        assert_eq!(job.status, EventStatus::Pending);
    }

    #[test]
    fn test_job_builder_methods() {
        let scheduled_time = Timestamp::now()
            .checked_add(jiff::SignedDuration::from_secs(3600))
            .unwrap();

        let job = DocumentJob::new_preprocessing(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "pdf".to_string(),
            None,
            vec![],
        )
        .with_priority(EventPriority::High)
        .with_max_retries(5)
        .with_timeout(Duration::from_secs(600))
        .scheduled_for(scheduled_time);

        assert_eq!(job.priority, EventPriority::High);
        assert_eq!(job.max_retries, 5);
        assert_eq!(job.timeout, Duration::from_secs(600));
        assert_eq!(job.scheduled_for, Some(scheduled_time));
    }

    #[test]
    fn test_job_priority_levels() {
        let mut job = DocumentJob::new_preprocessing(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "pdf".to_string(),
            None,
            vec![],
        );

        // Test all priority levels
        job = job.with_priority(EventPriority::Low);
        assert_eq!(job.priority, EventPriority::Low);

        job = job.with_priority(EventPriority::Normal);
        assert_eq!(job.priority, EventPriority::Normal);

        job = job.with_priority(EventPriority::High);
        assert_eq!(job.priority, EventPriority::High);

        job = job.with_priority(EventPriority::Critical);
        assert_eq!(job.priority, EventPriority::Critical);
    }

    #[test]
    fn test_job_retry_logic() {
        let mut job = DocumentJob::new_preprocessing(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "pdf".to_string(),
            None,
            vec![],
        )
        .with_max_retries(3);

        // Initially can retry
        assert!(job.can_retry());
        assert_eq!(job.retry_count, 0);

        // Increment retries
        job.increment_retry();
        assert_eq!(job.retry_count, 1);
        assert!(job.can_retry());

        job.increment_retry();
        assert_eq!(job.retry_count, 2);
        assert!(job.can_retry());

        job.increment_retry();
        assert_eq!(job.retry_count, 3);
        assert!(!job.can_retry()); // Max retries reached
    }
}
