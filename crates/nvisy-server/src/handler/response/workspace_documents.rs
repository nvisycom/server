//! Document response types.

use jiff::Timestamp;
use nvisy_postgres::model::{Blob, WorkspaceDocument as DocumentModel};
use nvisy_postgres::types::{DocumentKind, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Represents a document in responses.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDocument {
    /// Unique document identifier.
    pub id: Uuid,
    /// Handle of the workspace this document belongs to.
    pub workspace_slug: Handle,
    /// Display name.
    pub display_name: String,
    /// Original filename when uploaded.
    pub original_filename: String,
    /// File extension (without dot). Owned by the document.
    pub extension: String,
    /// Size in bytes. Resolved from the backing blob.
    pub size: i64,
    /// Lowercase hex-encoded SHA-256 of the document's plaintext content. Resolved
    /// from the backing blob.
    pub hash: String,
    /// The document's role (original or redacted).
    pub kind: DocumentKind,
    /// Account that uploaded/created the document.
    pub uploaded_by: AccountRef,
    /// Creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

impl WorkspaceDocument {
    /// Builds the response from a document and its backing blob (which carries the
    /// content-addressed fields: size, hash).
    pub fn from_model(
        document: DocumentModel,
        blob: &Blob,
        workspace_slug: Handle,
        uploaded_by: AccountRef,
    ) -> Self {
        Self {
            id: document.id,
            workspace_slug,
            display_name: document.display_name,
            original_filename: document.original_filename,
            extension: document.file_extension.clone(),
            size: blob.file_size_bytes,
            hash: hex::encode(&blob.content_hash),
            kind: document.kind,
            uploaded_by,
            created_at: document.created_at.into(),
            updated_at: document.updated_at.into(),
        }
    }
}

/// Result of a bulk document deletion.
///
/// The deletion is idempotent: `deleted` holds the ids that resolved to live
/// documents in the workspace and were removed, and `skipped` holds the requested
/// ids that did not — unknown, already deleted, in another workspace, or held by
/// an in-progress detection that still needs the document.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletedDocuments {
    /// Ids that were deleted.
    pub deleted: Vec<Uuid>,
    /// Requested ids that were skipped: unknown, already deleted, in another
    /// workspace, or held by an in-progress detection.
    pub skipped: Vec<Uuid>,
}

/// Response for document uploads (simple list without pagination).
pub type WorkspaceDocuments = Vec<WorkspaceDocument>;

/// Paginated response for document listing.
pub type WorkspaceDocumentsPage = Page<WorkspaceDocument>;
