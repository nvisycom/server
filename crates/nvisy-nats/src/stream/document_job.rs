//! Document job types for file processing pipeline.

use jiff::Timestamp;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::document_task::PredefinedTask;
use super::event::{EventPriority, EventStatus};

/// Preprocessing stage data.
///
/// Runs when a user uploads a file. Prepares the file for future processing:
/// - Format detection and validation
/// - File integrity checks
/// - Metadata extraction and fixes
/// - Thumbnail generation
/// - OCR for scanned documents
/// - Embedding generation for knowledge base / semantic search
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct PreprocessingData {
    /// Whether to validate and fix file metadata. Defaults to true.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub validate_metadata: bool,
    /// Whether to run OCR on the document. Defaults to true.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub run_ocr: bool,
    /// Whether to generate embeddings for semantic search. Defaults to true.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub generate_embeddings: bool,
    /// Whether to generate thumbnails for UI previews.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generate_thumbnails: Option<bool>,
}

impl Default for PreprocessingData {
    fn default() -> Self {
        Self {
            validate_metadata: true,
            run_ocr: true,
            generate_embeddings: true,
            generate_thumbnails: None,
        }
    }
}

/// Processing stage data.
///
/// Runs when a user requests changes to the document. Changes are typically
/// a collection of annotations (notes, highlights, comments) that need to be
/// applied using VLM pipelines.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ProcessingData {
    /// The main VLM prompt/instruction for processing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prompt: String,
    /// Additional context for the VLM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Annotation IDs to process. None means process all annotations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation_ids: Option<Vec<Uuid>>,
    /// Other files to use as context (e.g., "make this look like that").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_file_ids: Option<Vec<Uuid>>,
    /// Predefined processing tasks to apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<PredefinedTask>,
    /// Processing quality level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<ProcessingQuality>,
    /// Whether to process in chunks for large files. Defaults to false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub chunk_processing: bool,
    /// Custom processing parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_params: Option<serde_json::Value>,
}

/// Postprocessing stage data.
///
/// Runs when a user downloads the file. Prepares the final output:
/// - Format conversion to requested format
/// - Compression settings
/// - Cleanup of temporary artifacts
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct PostprocessingData {
    /// Target format for the output file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_format: Option<String>,
    /// Compression level for output file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression_level: Option<CompressionLevel>,
    /// Whether to burn annotations into the document vs keeping as metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flatten_annotations: Option<bool>,
    /// Cleanup tasks to perform.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup_tasks: Option<Vec<String>>,
}

/// Document processing stage with associated data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "stage", content = "data", rename_all = "snake_case")]
pub enum ProcessingStage {
    /// File upload: validation, OCR, embeddings.
    Preprocessing(PreprocessingData),
    /// User changes: apply annotations via VLM.
    Processing(ProcessingData),
    /// File download: format conversion.
    Postprocessing(PostprocessingData),
}

impl Default for ProcessingStage {
    fn default() -> Self {
        Self::Preprocessing(PreprocessingData::default())
    }
}

/// Processing quality level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProcessingQuality {
    /// Fast processing with lower quality.
    Fast,
    /// Balanced speed and quality.
    Balanced,
    /// High quality, slower processing.
    High,
}

/// Compression level for output files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CompressionLevel {
    /// No compression.
    None,
    /// Low compression, fast.
    Low,
    /// Medium compression, balanced.
    Medium,
    /// High compression, slower but smaller files.
    High,
    /// Maximum compression, slowest but smallest files.
    Maximum,
}

/// Document processing job.
///
/// Represents a unit of work in the document processing pipeline.
/// Each job targets a specific file and processing stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct DocumentJob {
    /// Unique job identifier (UUID v7 for time-ordering).
    pub id: Uuid,
    /// Database file ID to process.
    pub file_id: Uuid,
    /// Storage path in NATS object store (DocumentKey encoded).
    pub storage_path: String,
    /// File extension for format detection.
    pub file_extension: String,
    /// Processing stage with associated data.
    pub stage: ProcessingStage,
    /// Job priority.
    pub priority: EventPriority,
    /// Job status.
    pub status: EventStatus,
    /// When the job was created.
    pub created_at: Timestamp,
    /// NATS subject to publish result to (for internal job chaining).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback_subject: Option<String>,
    /// Idempotency key to prevent duplicate job processing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl DocumentJob {
    /// Creates a new preprocessing job (file upload).
    pub fn new(file_id: Uuid, storage_path: String, file_extension: String) -> Self {
        Self {
            id: Uuid::now_v7(),
            file_id,
            storage_path,
            file_extension,
            stage: ProcessingStage::default(),
            priority: EventPriority::Normal,
            status: EventStatus::Pending,
            created_at: Timestamp::now(),
            callback_subject: None,
            idempotency_key: None,
        }
    }

    /// Creates a new file processing job (compatibility constructor).
    pub fn new_file_processing(
        file_id: Uuid,
        _workspace_id: Uuid,
        _account_id: Uuid,
        storage_path: String,
        file_extension: String,
        _file_size_bytes: i64,
    ) -> Self {
        Self::new(file_id, storage_path, file_extension)
    }

    /// Creates a preprocessing job (file upload).
    pub fn preprocessing(
        file_id: Uuid,
        storage_path: String,
        file_extension: String,
        data: PreprocessingData,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            file_id,
            storage_path,
            file_extension,
            stage: ProcessingStage::Preprocessing(data),
            priority: EventPriority::Normal,
            status: EventStatus::Pending,
            created_at: Timestamp::now(),
            callback_subject: None,
            idempotency_key: None,
        }
    }

    /// Creates a processing job (user changes via annotations).
    pub fn processing(
        file_id: Uuid,
        storage_path: String,
        file_extension: String,
        data: ProcessingData,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            file_id,
            storage_path,
            file_extension,
            stage: ProcessingStage::Processing(data),
            priority: EventPriority::Normal,
            status: EventStatus::Pending,
            created_at: Timestamp::now(),
            callback_subject: None,
            idempotency_key: None,
        }
    }

    /// Creates a postprocessing job (file download).
    pub fn postprocessing(
        file_id: Uuid,
        storage_path: String,
        file_extension: String,
        data: PostprocessingData,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            file_id,
            storage_path,
            file_extension,
            stage: ProcessingStage::Postprocessing(data),
            priority: EventPriority::Normal,
            status: EventStatus::Pending,
            created_at: Timestamp::now(),
            callback_subject: None,
            idempotency_key: None,
        }
    }

    /// Sets the job priority.
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
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

    /// Returns the file ID.
    #[inline]
    pub fn file_id(&self) -> Uuid {
        self.file_id
    }

    /// Returns the storage path.
    #[inline]
    pub fn storage_path(&self) -> &str {
        &self.storage_path
    }

    /// Returns the file extension.
    #[inline]
    pub fn file_extension(&self) -> &str {
        &self.file_extension
    }

    /// Returns the processing stage.
    #[inline]
    pub fn stage(&self) -> &ProcessingStage {
        &self.stage
    }

    /// Checks if the job is in preprocessing stage.
    pub fn is_preprocessing(&self) -> bool {
        matches!(self.stage, ProcessingStage::Preprocessing(_))
    }

    /// Checks if the job is in processing stage.
    pub fn is_processing(&self) -> bool {
        matches!(self.stage, ProcessingStage::Processing(_))
    }

    /// Checks if the job is in postprocessing stage.
    pub fn is_postprocessing(&self) -> bool {
        matches!(self.stage, ProcessingStage::Postprocessing(_))
    }

    /// Returns job age since creation.
    pub fn age(&self) -> std::time::Duration {
        let now = Timestamp::now();
        let signed_dur = now.duration_since(self.created_at);
        std::time::Duration::from_secs(signed_dur.as_secs().max(0) as u64)
    }
}

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_job_new() {
        let file_id = Uuid::now_v7();
        let job = DocumentJob::new(file_id, "storage/path".to_string(), "pdf".to_string());

        assert_eq!(job.file_id(), file_id);
        assert_eq!(job.storage_path(), "storage/path");
        assert_eq!(job.file_extension(), "pdf");
        assert!(job.is_preprocessing());
    }

    #[test]
    fn test_preprocessing_defaults() {
        let data = PreprocessingData::default();
        assert!(data.validate_metadata);
        assert!(data.run_ocr);
        assert!(data.generate_embeddings);
        assert!(data.generate_thumbnails.is_none());
    }

    #[test]
    fn test_preprocessing_serialization_skips_defaults() {
        let data = PreprocessingData::default();
        let json = serde_json::to_string(&data).unwrap();
        // Should be minimal since defaults are skipped
        assert_eq!(json, "{}");

        // Parsing empty object should give defaults
        let parsed: PreprocessingData = serde_json::from_str("{}").unwrap();
        assert!(parsed.validate_metadata);
        assert!(parsed.run_ocr);
        assert!(parsed.generate_embeddings);
    }

    #[test]
    fn test_document_job_processing_with_prompt() {
        let file_id = Uuid::now_v7();

        let job = DocumentJob::processing(
            file_id,
            "storage/path".to_string(),
            "pdf".to_string(),
            ProcessingData {
                prompt: "Apply the highlighted changes".to_string(),
                context: Some("This is a legal document".to_string()),
                annotation_ids: None, // Process all annotations
                tasks: vec![PredefinedTask::Proofread],
                ..Default::default()
            },
        );

        assert!(job.is_processing());
        if let ProcessingStage::Processing(data) = job.stage() {
            assert_eq!(data.prompt, "Apply the highlighted changes");
            assert_eq!(data.context, Some("This is a legal document".to_string()));
            assert!(data.annotation_ids.is_none());
            assert_eq!(data.tasks.len(), 1);
        }
    }

    #[test]
    fn test_predefined_task_redact() {
        let task = PredefinedTask::Redact {
            patterns: vec!["email".to_string(), "phone".to_string()],
        };

        let json = serde_json::to_string(&task).unwrap();
        let parsed: PredefinedTask = serde_json::from_str(&json).unwrap();
        assert_eq!(task, parsed);
    }

    #[test]
    fn test_predefined_task_translate() {
        let task = PredefinedTask::Translate {
            target_language: "es".to_string(),
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("translate"));
        assert!(json.contains("es"));
    }

    #[test]
    fn test_document_job_postprocessing() {
        let file_id = Uuid::now_v7();
        let job = DocumentJob::postprocessing(
            file_id,
            "storage/path".to_string(),
            "pdf".to_string(),
            PostprocessingData {
                target_format: Some("docx".to_string()),
                compression_level: Some(CompressionLevel::Medium),
                ..Default::default()
            },
        );

        assert!(job.is_postprocessing());
        if let ProcessingStage::Postprocessing(data) = job.stage() {
            assert_eq!(data.target_format, Some("docx".to_string()));
            assert_eq!(data.compression_level, Some(CompressionLevel::Medium));
        }
    }

    #[test]
    fn test_job_with_callback_and_idempotency() {
        let file_id = Uuid::now_v7();
        let job = DocumentJob::new(file_id, "storage/path".to_string(), "pdf".to_string())
            .with_callback("results.preprocessing")
            .with_idempotency_key("upload-123");

        assert_eq!(
            job.callback_subject,
            Some("results.preprocessing".to_string())
        );
        assert_eq!(job.idempotency_key, Some("upload-123".to_string()));
    }

    #[test]
    fn test_processing_stage_serialization() {
        let stage = ProcessingStage::Preprocessing(PreprocessingData {
            validate_metadata: true,
            run_ocr: true,
            generate_embeddings: true,
            generate_thumbnails: Some(true),
        });

        let json = serde_json::to_string(&stage).unwrap();
        let parsed: ProcessingStage = serde_json::from_str(&json).unwrap();

        assert_eq!(stage, parsed);
    }

    #[test]
    fn test_compression_level_serialization() {
        let level = CompressionLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"high\"");

        let parsed: CompressionLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, CompressionLevel::High);
    }
}
