//! Workspace document model for PostgreSQL database operations.
//!
//! A document is the human-facing file: an uploaded/imported original, or a
//! redacted output. Its bytes live in a [`Blob`](crate::model::Blob); the
//! document row carries the human-facing metadata (name, kind, creator) and the
//! soft-delete lifecycle. Machine byproducts (audits, intermediates) are not
//! documents — they reference blobs from their own tables.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_documents;
use crate::types::DocumentKind;

/// A human-facing document: an original upload/import or a redacted output.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceDocument {
    /// Unique document identifier.
    pub id: Uuid,
    /// Workspace this document belongs to.
    pub workspace_id: Uuid,
    /// Account that created (uploaded or produced) this document.
    pub account_id: Uuid,
    /// Blob holding this document's bytes.
    pub blob_id: Uuid,
    /// The document's role (original or redacted).
    pub kind: DocumentKind,
    /// Human-readable name for display.
    pub display_name: String,
    /// Original filename when uploaded or imported.
    pub original_filename: String,
    /// File extension (without the dot); codec and content-type dispatch use it.
    pub file_extension: String,
    /// Document metadata (JSON).
    pub metadata: serde_json::Value,
    /// When the document was created.
    pub created_at: Timestamp,
    /// When the document was last updated.
    pub updated_at: Timestamp,
    /// When the document was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new document.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceDocument {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID (required).
    pub account_id: Uuid,
    /// Blob holding the document's bytes (required).
    pub blob_id: Uuid,
    /// The document's role.
    pub kind: Option<DocumentKind>,
    /// Display name.
    pub display_name: Option<String>,
    /// Original filename.
    pub original_filename: Option<String>,
    /// File extension (without the dot).
    pub file_extension: Option<String>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

impl NewWorkspaceDocument {
    /// A minimal `original` document for `workspace_id` backed by `blob_id`, for
    /// tests. The name and kind take their database defaults.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, account_id: Uuid, blob_id: Uuid) -> Self {
        Self {
            workspace_id,
            account_id,
            blob_id,
            ..Default::default()
        }
    }
}

/// Data for updating a document.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceDocument {
    /// Display name.
    pub display_name: Option<String>,
    /// The document's role.
    pub kind: Option<DocumentKind>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}
