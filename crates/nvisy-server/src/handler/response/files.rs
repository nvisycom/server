//! File response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceFile as FileModel;
use nvisy_postgres::types::{FileKind, Handle};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};

/// Represents a file in responses.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// Unique file identifier.
    pub id: Uuid,
    /// Handle of the workspace this file belongs to.
    pub workspace_slug: Handle,
    /// Display name.
    pub display_name: String,
    /// Original filename when uploaded.
    pub original_filename: String,
    /// File extension (without dot).
    pub file_extension: String,
    /// File size in bytes.
    pub file_size: i64,
    /// The file's role (original, redacted, audit).
    pub file_kind: FileKind,
    /// Account that uploaded/created the file.
    pub uploaded_by: AccountRef,
    /// Version number (1 for original, higher for newer versions).
    pub version_number: i32,
    /// Parent file ID if this is a newer version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    /// Creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

impl File {
    pub fn from_model(file: FileModel, workspace_slug: Handle, uploaded_by: AccountRef) -> Self {
        Self {
            id: file.id,
            workspace_slug,
            display_name: file.display_name,
            original_filename: file.original_filename,
            file_extension: file.file_extension,
            file_size: file.file_size_bytes,
            file_kind: file.file_kind,
            uploaded_by,
            version_number: file.version_number,
            parent_id: file.parent_id,
            created_at: file.created_at.into(),
            updated_at: file.updated_at.into(),
        }
    }
}

/// Response for file uploads (simple list without pagination).
pub type Files = Vec<File>;

/// Paginated response for file listing.
pub type FilesPage = Page<File>;
