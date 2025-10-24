//! Document job types and shared structures.

use std::time::Duration;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{JobPriority, JobStatus};

/// Document processing stages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStage {
    /// Initial document preprocessing
    Preprocessing,
    /// Main document processing
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

/// Document processing job
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentJob {
    pub id: Uuid,
    pub payload: DocumentJobPayload,
    pub priority: JobPriority,
    pub max_retries: u32,
    pub retry_count: u32,
    pub timeout: Duration,
    pub created_at: Timestamp,
    pub scheduled_for: Option<Timestamp>,
    pub status: JobStatus,
}

impl DocumentJob {
    /// Create a new preprocessing job
    pub fn new_preprocessing(
        document_id: Uuid,
        account_id: Uuid,
        source_format: String,
        target_format: Option<String>,
        validation_rules: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            payload: DocumentJobPayload {
                document_id,
                account_id,
                stage: ProcessingStage::Preprocessing,
                source_format: Some(source_format),
                target_format,
                validation_rules: Some(validation_rules),
                processing_type: None,
                options: None,
                cleanup_tasks: None,
                finalization_steps: None,
            },
            priority: JobPriority::Normal,
            max_retries: 3,
            retry_count: 0,
            timeout: Duration::from_secs(300),
            created_at: Timestamp::now(),
            scheduled_for: None,
            status: JobStatus::Pending,
        }
    }

    /// Create a new processing job
    pub fn new_processing(
        document_id: Uuid,
        account_id: Uuid,
        processing_type: ProcessingType,
        options: ProcessingOptions,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            payload: DocumentJobPayload {
                document_id,
                account_id,
                stage: ProcessingStage::Processing,
                source_format: None,
                target_format: None,
                validation_rules: None,
                processing_type: Some(processing_type),
                options: Some(options),
                cleanup_tasks: None,
                finalization_steps: None,
            },
            priority: JobPriority::Normal,
            max_retries: 3,
            retry_count: 0,
            timeout: Duration::from_secs(300),
            created_at: Timestamp::now(),
            scheduled_for: None,
            status: JobStatus::Pending,
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
            id: Uuid::new_v4(),
            payload: DocumentJobPayload {
                document_id,
                account_id,
                stage: ProcessingStage::Postprocessing,
                source_format: None,
                target_format: None,
                validation_rules: None,
                processing_type: None,
                options: None,
                cleanup_tasks: Some(cleanup_tasks),
                finalization_steps: Some(finalization_steps),
            },
            priority: JobPriority::Normal,
            max_retries: 3,
            retry_count: 0,
            timeout: Duration::from_secs(300),
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
        assert_eq!(job.priority, JobPriority::Normal);
        assert_eq!(job.status, JobStatus::Pending);
        assert!(job.can_retry());
        assert_eq!(job.payload.source_format, Some("pdf".to_string()));
        assert_eq!(job.payload.target_format, Some("docx".to_string()));
        assert_eq!(
            job.payload.validation_rules,
            Some(vec!["validate_structure".to_string()])
        );
    }

    #[test]
    fn test_processing_job_creation() {
        let job = DocumentJob::new_processing(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ProcessingType::Ocr,
            ProcessingOptions {
                quality: Some("high".to_string()),
                language: Some("en".to_string()),
                custom_params: None,
            },
        );

        assert_eq!(job.payload.stage, ProcessingStage::Processing);
        assert_eq!(job.payload.processing_type, Some(ProcessingType::Ocr));
        assert_eq!(
            job.payload.options.as_ref().unwrap().quality,
            Some("high".to_string())
        );
        assert_eq!(
            job.payload.options.as_ref().unwrap().language,
            Some("en".to_string())
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
        assert_eq!(job.priority, JobPriority::Normal);
        assert_eq!(job.status, JobStatus::Pending);
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
        .with_priority(JobPriority::High)
        .with_max_retries(5)
        .with_timeout(Duration::from_secs(600))
        .scheduled_for(scheduled_time);

        assert_eq!(job.priority, JobPriority::High);
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
        job = job.with_priority(JobPriority::Low);
        assert_eq!(job.priority, JobPriority::Low);

        job = job.with_priority(JobPriority::Normal);
        assert_eq!(job.priority, JobPriority::Normal);

        job = job.with_priority(JobPriority::High);
        assert_eq!(job.priority, JobPriority::High);

        job = job.with_priority(JobPriority::Critical);
        assert_eq!(job.priority, JobPriority::Critical);
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
