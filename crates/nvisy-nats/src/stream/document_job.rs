//! Document job types for file processing pipeline.

use jiff::Timestamp;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::document_task::PredefinedTask;
use super::event::EventPriority;

/// Stream name for document jobs.
pub const STREAM_NAME: &str = "DOCUMENT_JOBS";

/// Marker trait for document processing stages.
///
/// Each stage represents a distinct phase in the document processing pipeline,
/// with its own stream subject for NATS routing.
pub trait Stage: Serialize + DeserializeOwned + Clone + Send + Sync + 'static {
    /// Stage name for logging and debugging.
    const NAME: &'static str;
    /// NATS stream subject suffix for this stage.
    const SUBJECT: &'static str;
}

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

impl Stage for PreprocessingData {
    const NAME: &'static str = "preprocessing";
    const SUBJECT: &'static str = "preprocessing";
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

impl Stage for ProcessingData {
    const NAME: &'static str = "processing";
    const SUBJECT: &'static str = "processing";
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

impl Stage for PostprocessingData {
    const NAME: &'static str = "postprocessing";
    const SUBJECT: &'static str = "postprocessing";
}

/// Processing quality level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub enum CompressionLevel {
    /// No compression.
    None,
    /// Medium compression, balanced.
    Normal,
    /// High compression, slower but smaller files.
    High,
}

/// Document processing job.
///
/// Represents a unit of work in the document processing pipeline.
/// Each job targets a specific file and is typed by its processing stage.
///
/// The generic parameter `S` determines the stage (preprocessing, processing,
/// or postprocessing), enabling compile-time type safety and stage-specific
/// stream routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(bound = "")]
pub struct DocumentJob<S: Stage> {
    /// Unique job identifier (UUID v7 for time-ordering).
    pub id: Uuid,
    /// Database file ID to process.
    pub file_id: Uuid,
    /// Storage path in NATS object store (DocumentKey encoded).
    pub object_key: String,
    /// File extension for format detection.
    pub file_extension: String,
    /// Stage-specific data.
    pub data: S,
    /// Job priority.
    pub priority: EventPriority,
    /// When the job was created.
    pub created_at: Timestamp,
    /// NATS subject to publish result to (for internal job chaining).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback_subject: Option<String>,
    /// Idempotency key to prevent duplicate job processing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl<S: Stage> DocumentJob<S> {
    /// Creates a new document job with the given stage data.
    pub fn new(file_id: Uuid, storage_path: String, file_extension: String, data: S) -> Self {
        Self {
            id: Uuid::now_v7(),
            file_id,
            object_key: storage_path,
            file_extension,
            data,
            priority: EventPriority::Normal,
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
        &self.object_key
    }

    /// Returns the file extension.
    #[inline]
    pub fn file_extension(&self) -> &str {
        &self.file_extension
    }

    /// Returns a reference to the stage data.
    #[inline]
    pub fn data(&self) -> &S {
        &self.data
    }

    /// Returns the stage name.
    #[inline]
    pub fn stage_name(&self) -> &'static str {
        S::NAME
    }

    /// Returns the stream subject for this job's stage.
    #[inline]
    pub fn subject(&self) -> &'static str {
        S::SUBJECT
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
    fn test_preprocessing_job_new() {
        let file_id = Uuid::now_v7();
        let job = DocumentJob::new(
            file_id,
            "storage/path".to_string(),
            "pdf".to_string(),
            PreprocessingData::default(),
        );

        assert_eq!(job.file_id(), file_id);
        assert_eq!(job.storage_path(), "storage/path");
        assert_eq!(job.file_extension(), "pdf");
        assert_eq!(job.stage_name(), "preprocessing");
        assert_eq!(job.subject(), "preprocessing");
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
    fn test_processing_job_with_prompt() {
        let file_id = Uuid::now_v7();

        let job = DocumentJob::new(
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

        assert_eq!(job.stage_name(), "processing");
        assert_eq!(job.data().prompt, "Apply the highlighted changes");
        assert_eq!(
            job.data().context,
            Some("This is a legal document".to_string())
        );
        assert!(job.data().annotation_ids.is_none());
        assert_eq!(job.data().tasks.len(), 1);
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
    fn test_postprocessing_job() {
        let file_id = Uuid::now_v7();
        let job = DocumentJob::new(
            file_id,
            "storage/path".to_string(),
            "pdf".to_string(),
            PostprocessingData {
                target_format: Some("docx".to_string()),
                compression_level: Some(CompressionLevel::Normal),
                ..Default::default()
            },
        );

        assert_eq!(job.stage_name(), "postprocessing");
        assert_eq!(job.data().target_format, Some("docx".to_string()));
        assert_eq!(job.data().compression_level, Some(CompressionLevel::Normal));
    }

    #[test]
    fn test_job_with_callback_and_idempotency() {
        let file_id = Uuid::now_v7();
        let job = DocumentJob::new(
            file_id,
            "storage/path".to_string(),
            "pdf".to_string(),
            PreprocessingData::default(),
        )
        .with_callback("results.preprocessing")
        .with_idempotency_key("upload-123");

        assert_eq!(
            job.callback_subject,
            Some("results.preprocessing".to_string())
        );
        assert_eq!(job.idempotency_key, Some("upload-123".to_string()));
    }

    #[test]
    fn test_job_serialization_roundtrip() {
        let file_id = Uuid::now_v7();
        let job = DocumentJob::new(
            file_id,
            "storage/path".to_string(),
            "pdf".to_string(),
            PreprocessingData {
                validate_metadata: true,
                run_ocr: true,
                generate_embeddings: true,
                generate_thumbnails: Some(true),
            },
        );

        let json = serde_json::to_string(&job).unwrap();
        let parsed: DocumentJob<PreprocessingData> = serde_json::from_str(&json).unwrap();

        assert_eq!(job.file_id, parsed.file_id);
        assert_eq!(job.data, parsed.data);
    }

    #[test]
    fn test_compression_level_serialization() {
        let level = CompressionLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"high\"");

        let parsed: CompressionLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, CompressionLevel::High);
    }

    #[test]
    fn test_processing_quality_serialization() {
        let quality = ProcessingQuality::Fast;
        let json = serde_json::to_string(&quality).unwrap();
        assert_eq!(json, "\"fast\"");

        let parsed: ProcessingQuality = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ProcessingQuality::Fast);
    }

    #[test]
    fn test_stage_constants() {
        assert_eq!(PreprocessingData::NAME, "preprocessing");
        assert_eq!(PreprocessingData::SUBJECT, "preprocessing");

        assert_eq!(ProcessingData::NAME, "processing");
        assert_eq!(ProcessingData::SUBJECT, "processing");

        assert_eq!(PostprocessingData::NAME, "postprocessing");
        assert_eq!(PostprocessingData::SUBJECT, "postprocessing");
    }
}
