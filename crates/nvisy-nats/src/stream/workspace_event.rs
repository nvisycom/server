//! Workspace event stream for real-time WebSocket communication.
//!
//! This module provides NATS-based pub/sub for workspace WebSocket messages,
//! enabling distributed real-time communication across multiple server instances.

use jiff::Timestamp;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Member joined the workspace event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct JoinEvent {
    pub account_id: Uuid,
    pub display_name: String,
    pub timestamp: Timestamp,
}

/// Member left the workspace event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveEvent {
    pub account_id: Uuid,
    pub timestamp: Timestamp,
}

/// Document content update event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DocumentUpdateEvent {
    pub document_id: Uuid,
    pub version: u32,
    pub updated_by: Uuid,
    pub timestamp: Timestamp,
}

/// Document created event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DocumentCreatedEvent {
    pub document_id: Uuid,
    pub display_name: String,
    pub created_by: Uuid,
    pub timestamp: Timestamp,
}

/// Document deleted event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DocumentDeletedEvent {
    pub document_id: Uuid,
    pub deleted_by: Uuid,
    pub timestamp: Timestamp,
}

/// Type of preprocessing operation completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PreprocessingType {
    /// File metadata validation completed.
    Validation,
    /// OCR text extraction completed.
    Ocr,
    /// Embeddings generation completed.
    Embeddings,
    /// Thumbnail generation completed.
    Thumbnails,
    /// All preprocessing steps completed.
    Complete,
}

/// File preprocessing completed event.
///
/// Emitted when a preprocessing step (validation, OCR, embeddings) completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FilePreprocessedEvent {
    pub file_id: Uuid,
    pub document_id: Uuid,
    pub preprocessing_type: PreprocessingType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    pub timestamp: Timestamp,
}

/// Type of transformation applied to the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TransformationType {
    /// Content was redacted.
    Redaction,
    /// Content was summarized.
    Summary,
    /// Content was translated.
    Translation,
    /// Information was extracted.
    Extraction,
    /// Information was inserted.
    Insertion,
    /// Content was reformatted.
    Reformat,
    /// Content was proofread.
    Proofread,
    /// Table of contents was generated.
    TableOfContents,
    /// File was split into multiple files.
    Split,
    /// Multiple files were merged.
    Merge,
    /// Custom VLM-based transformation.
    Custom,
}

/// File transformed event.
///
/// Emitted when a document processing transformation completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileTransformedEvent {
    pub file_id: Uuid,
    pub document_id: Uuid,
    pub transformation_type: TransformationType,
    /// For split operations, the resulting file IDs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_file_ids: Option<Vec<Uuid>>,
    /// Human-readable summary of the transformation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub transformed_by: Uuid,
    pub timestamp: Timestamp,
}

/// Type of postprocessing operation completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PostprocessingType {
    /// Format conversion completed.
    Conversion,
    /// Compression completed.
    Compression,
    /// Annotations flattened into document.
    FlattenAnnotations,
    /// All postprocessing steps completed.
    Complete,
}

/// File postprocessed event.
///
/// Emitted when a postprocessing step (conversion, compression) completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FilePostprocessedEvent {
    pub file_id: Uuid,
    pub document_id: Uuid,
    pub postprocessing_type: PostprocessingType,
    /// The output format if conversion was performed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    pub timestamp: Timestamp,
}

/// Job processing stage for progress tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum JobStage {
    Preprocessing,
    Processing,
    Postprocessing,
}

/// Job progress event.
///
/// Emitted periodically during long-running jobs to indicate progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct JobProgressEvent {
    pub job_id: Uuid,
    pub file_id: Uuid,
    pub document_id: Uuid,
    pub stage: JobStage,
    /// Progress percentage (0-100).
    pub progress: u8,
    /// Current operation being performed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_operation: Option<String>,
    pub timestamp: Timestamp,
}

/// Job completed event.
///
/// Emitted when an entire document processing job completes successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct JobCompletedEvent {
    pub job_id: Uuid,
    pub file_id: Uuid,
    pub document_id: Uuid,
    /// The final output file ID (may differ from input if transformations created new files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file_id: Option<Uuid>,
    pub timestamp: Timestamp,
}

/// Job failed event.
///
/// Emitted when a document processing job fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct JobFailedEvent {
    pub job_id: Uuid,
    pub file_id: Uuid,
    pub document_id: Uuid,
    pub stage: JobStage,
    pub error_code: String,
    pub error_message: String,
    pub timestamp: Timestamp,
}

/// Member presence update event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberPresenceEvent {
    pub account_id: Uuid,
    pub is_online: bool,
    pub timestamp: Timestamp,
}

/// Member added to workspace event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberAddedEvent {
    pub account_id: Uuid,
    pub display_name: String,
    pub member_role: String,
    pub added_by: Uuid,
    pub timestamp: Timestamp,
}

/// Member removed from workspace event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberRemovedEvent {
    pub account_id: Uuid,
    pub removed_by: Uuid,
    pub timestamp: Timestamp,
}

/// Workspace settings updated event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUpdatedEvent {
    pub display_name: Option<String>,
    pub updated_by: Uuid,
    pub timestamp: Timestamp,
}

/// Typing indicator event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct TypingEvent {
    pub account_id: Uuid,
    pub document_id: Option<Uuid>,
    pub timestamp: Timestamp,
}

/// Error event from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvent {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// WebSocket message types for workspace communication.
///
/// All messages are serialized as JSON with a `type` field that identifies
/// the message variant. This enables type-safe message handling on both
/// client and server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WorkspaceWsMessage {
    /// Client announces presence in the workspace.
    Join(JoinEvent),

    /// Client leaves the workspace.
    Leave(LeaveEvent),

    /// Document content update notification.
    DocumentUpdate(DocumentUpdateEvent),

    /// Document creation notification.
    DocumentCreated(DocumentCreatedEvent),

    /// Document deletion notification.
    DocumentDeleted(DocumentDeletedEvent),

    /// File preprocessing step completed (validation, OCR, embeddings).
    FilePreprocessed(FilePreprocessedEvent),

    /// File transformation completed (redaction, translation, etc.).
    FileTransformed(FileTransformedEvent),

    /// File postprocessing step completed (conversion, compression).
    FilePostprocessed(FilePostprocessedEvent),

    /// Job progress update.
    JobProgress(JobProgressEvent),

    /// Job completed successfully.
    JobCompleted(JobCompletedEvent),

    /// Job failed.
    JobFailed(JobFailedEvent),

    /// Member presence update.
    MemberPresence(MemberPresenceEvent),

    /// Member added to workspace.
    MemberAdded(MemberAddedEvent),

    /// Member removed from workspace.
    MemberRemoved(MemberRemovedEvent),

    /// Workspace settings updated.
    WorkspaceUpdated(WorkspaceUpdatedEvent),

    /// Typing indicator.
    Typing(TypingEvent),

    /// Error message from server.
    Error(ErrorEvent),
}

impl WorkspaceWsMessage {
    /// Creates an error message with the given code and message.
    #[inline]
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error(ErrorEvent {
            code: code.into(),
            message: message.into(),
            details: None,
        })
    }

    /// Creates an error message with additional details.
    #[inline]
    pub fn error_with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self::Error(ErrorEvent {
            code: code.into(),
            message: message.into(),
            details: Some(details.into()),
        })
    }

    /// Get the account ID associated with this message, if any.
    pub fn account_id(&self) -> Option<Uuid> {
        match self {
            Self::Join(e) => Some(e.account_id),
            Self::Leave(e) => Some(e.account_id),
            Self::DocumentUpdate(e) => Some(e.updated_by),
            Self::DocumentCreated(e) => Some(e.created_by),
            Self::DocumentDeleted(e) => Some(e.deleted_by),
            Self::FilePreprocessed(_) => None,
            Self::FileTransformed(e) => Some(e.transformed_by),
            Self::FilePostprocessed(_) => None,
            Self::JobProgress(_) => None,
            Self::JobCompleted(_) => None,
            Self::JobFailed(_) => None,
            Self::MemberPresence(e) => Some(e.account_id),
            Self::MemberAdded(e) => Some(e.account_id),
            Self::MemberRemoved(e) => Some(e.account_id),
            Self::WorkspaceUpdated(e) => Some(e.updated_by),
            Self::Typing(e) => Some(e.account_id),
            Self::Error(_) => None,
        }
    }

    /// Get the timestamp of this message.
    pub fn timestamp(&self) -> Option<Timestamp> {
        match self {
            Self::Join(e) => Some(e.timestamp),
            Self::Leave(e) => Some(e.timestamp),
            Self::DocumentUpdate(e) => Some(e.timestamp),
            Self::DocumentCreated(e) => Some(e.timestamp),
            Self::DocumentDeleted(e) => Some(e.timestamp),
            Self::FilePreprocessed(e) => Some(e.timestamp),
            Self::FileTransformed(e) => Some(e.timestamp),
            Self::FilePostprocessed(e) => Some(e.timestamp),
            Self::JobProgress(e) => Some(e.timestamp),
            Self::JobCompleted(e) => Some(e.timestamp),
            Self::JobFailed(e) => Some(e.timestamp),
            Self::MemberPresence(e) => Some(e.timestamp),
            Self::MemberAdded(e) => Some(e.timestamp),
            Self::MemberRemoved(e) => Some(e.timestamp),
            Self::WorkspaceUpdated(e) => Some(e.timestamp),
            Self::Typing(e) => Some(e.timestamp),
            Self::Error(_) => None,
        }
    }

    /// Create a join event.
    pub fn join(account_id: Uuid, display_name: impl Into<String>) -> Self {
        Self::Join(JoinEvent {
            account_id,
            display_name: display_name.into(),
            timestamp: Timestamp::now(),
        })
    }

    /// Create a leave event.
    pub fn leave(account_id: Uuid) -> Self {
        Self::Leave(LeaveEvent {
            account_id,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a typing event.
    pub fn typing(account_id: Uuid, document_id: Option<Uuid>) -> Self {
        Self::Typing(TypingEvent {
            account_id,
            document_id,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a document update event.
    pub fn document_update(document_id: Uuid, version: u32, updated_by: Uuid) -> Self {
        Self::DocumentUpdate(DocumentUpdateEvent {
            document_id,
            version,
            updated_by,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a document created event.
    pub fn document_created(
        document_id: Uuid,
        display_name: impl Into<String>,
        created_by: Uuid,
    ) -> Self {
        Self::DocumentCreated(DocumentCreatedEvent {
            document_id,
            display_name: display_name.into(),
            created_by,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a document deleted event.
    pub fn document_deleted(document_id: Uuid, deleted_by: Uuid) -> Self {
        Self::DocumentDeleted(DocumentDeletedEvent {
            document_id,
            deleted_by,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a file preprocessed event.
    pub fn file_preprocessed(
        file_id: Uuid,
        document_id: Uuid,
        preprocessing_type: PreprocessingType,
    ) -> Self {
        Self::FilePreprocessed(FilePreprocessedEvent {
            file_id,
            document_id,
            preprocessing_type,
            details: None,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a file transformed event.
    pub fn file_transformed(
        file_id: Uuid,
        document_id: Uuid,
        transformation_type: TransformationType,
        transformed_by: Uuid,
    ) -> Self {
        Self::FileTransformed(FileTransformedEvent {
            file_id,
            document_id,
            transformation_type,
            result_file_ids: None,
            summary: None,
            transformed_by,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a file postprocessed event.
    pub fn file_postprocessed(
        file_id: Uuid,
        document_id: Uuid,
        postprocessing_type: PostprocessingType,
    ) -> Self {
        Self::FilePostprocessed(FilePostprocessedEvent {
            file_id,
            document_id,
            postprocessing_type,
            output_format: None,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a job progress event.
    pub fn job_progress(
        job_id: Uuid,
        file_id: Uuid,
        document_id: Uuid,
        stage: JobStage,
        progress: u8,
    ) -> Self {
        Self::JobProgress(JobProgressEvent {
            job_id,
            file_id,
            document_id,
            stage,
            progress: progress.min(100),
            current_operation: None,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a job completed event.
    pub fn job_completed(job_id: Uuid, file_id: Uuid, document_id: Uuid) -> Self {
        Self::JobCompleted(JobCompletedEvent {
            job_id,
            file_id,
            document_id,
            output_file_id: None,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a job failed event.
    pub fn job_failed(
        job_id: Uuid,
        file_id: Uuid,
        document_id: Uuid,
        stage: JobStage,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> Self {
        Self::JobFailed(JobFailedEvent {
            job_id,
            file_id,
            document_id,
            stage,
            error_code: error_code.into(),
            error_message: error_message.into(),
            timestamp: Timestamp::now(),
        })
    }

    /// Create a member presence event.
    pub fn member_presence(account_id: Uuid, is_online: bool) -> Self {
        Self::MemberPresence(MemberPresenceEvent {
            account_id,
            is_online,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a member added event.
    pub fn member_added(
        account_id: Uuid,
        display_name: impl Into<String>,
        member_role: impl Into<String>,
        added_by: Uuid,
    ) -> Self {
        Self::MemberAdded(MemberAddedEvent {
            account_id,
            display_name: display_name.into(),
            member_role: member_role.into(),
            added_by,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a member removed event.
    pub fn member_removed(account_id: Uuid, removed_by: Uuid) -> Self {
        Self::MemberRemoved(MemberRemovedEvent {
            account_id,
            removed_by,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a workspace updated event.
    pub fn workspace_updated(display_name: Option<String>, updated_by: Uuid) -> Self {
        Self::WorkspaceUpdated(WorkspaceUpdatedEvent {
            display_name,
            updated_by,
            timestamp: Timestamp::now(),
        })
    }
}

/// Workspace event envelope for NATS publishing.
///
/// Wraps the WebSocket message with metadata for routing and filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct WorkspaceEvent {
    /// The workspace this event belongs to.
    pub workspace_id: Uuid,

    /// The WebSocket message payload.
    pub message: WorkspaceWsMessage,

    /// When this event was created.
    pub created_at: Timestamp,
}

impl WorkspaceEvent {
    /// Create a new workspace event.
    pub fn new(workspace_id: Uuid, message: WorkspaceWsMessage) -> Self {
        Self {
            workspace_id,
            message,
            created_at: Timestamp::now(),
        }
    }
}
