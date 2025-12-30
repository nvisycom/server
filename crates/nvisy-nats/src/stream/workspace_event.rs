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

/// File processed event (OCR, text extraction, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileProcessedEvent {
    pub file_id: Uuid,
    pub document_id: Uuid,
    pub processing_type: String,
    pub processed_by: Option<Uuid>,
    pub timestamp: Timestamp,
}

/// File redacted event (sensitive content removed).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileRedactedEvent {
    pub file_id: Uuid,
    pub document_id: Uuid,
    pub redaction_count: u32,
    pub redacted_by: Uuid,
    pub timestamp: Timestamp,
}

/// File verified event (authenticity check, virus scan, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileVerifiedEvent {
    pub file_id: Uuid,
    pub document_id: Uuid,
    pub verification_type: String,
    pub verification_status: String,
    pub verified_by: Option<Uuid>,
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

    /// File processing completion notification.
    FileProcessed(FileProcessedEvent),

    /// File redaction completion notification.
    FileRedacted(FileRedactedEvent),

    /// File verification completion notification.
    FileVerified(FileVerifiedEvent),

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
            Self::FileProcessed(e) => e.processed_by,
            Self::FileRedacted(e) => Some(e.redacted_by),
            Self::FileVerified(e) => e.verified_by,
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
            Self::FileProcessed(e) => Some(e.timestamp),
            Self::FileRedacted(e) => Some(e.timestamp),
            Self::FileVerified(e) => Some(e.timestamp),
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

    /// Create a file processed event.
    pub fn file_processed(
        file_id: Uuid,
        document_id: Uuid,
        processing_type: impl Into<String>,
        processed_by: Option<Uuid>,
    ) -> Self {
        Self::FileProcessed(FileProcessedEvent {
            file_id,
            document_id,
            processing_type: processing_type.into(),
            processed_by,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a file redacted event.
    pub fn file_redacted(
        file_id: Uuid,
        document_id: Uuid,
        redaction_count: u32,
        redacted_by: Uuid,
    ) -> Self {
        Self::FileRedacted(FileRedactedEvent {
            file_id,
            document_id,
            redaction_count,
            redacted_by,
            timestamp: Timestamp::now(),
        })
    }

    /// Create a file verified event.
    pub fn file_verified(
        file_id: Uuid,
        document_id: Uuid,
        verification_type: impl Into<String>,
        verification_status: impl Into<String>,
        verified_by: Option<Uuid>,
    ) -> Self {
        Self::FileVerified(FileVerifiedEvent {
            file_id,
            document_id,
            verification_type: verification_type.into(),
            verification_status: verification_status.into(),
            verified_by,
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
