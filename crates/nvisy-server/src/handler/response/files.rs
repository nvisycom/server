//! File response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceFile as FileModel;
use nvisy_postgres::types::FileSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Represents a file in responses.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// Unique file identifier.
    pub id: Uuid,
    /// Workspace this file belongs to.
    pub workspace_id: Uuid,
    /// Display name.
    pub display_name: String,
    /// Original filename when uploaded.
    pub original_filename: String,
    /// File extension (without dot).
    pub file_extension: String,
    /// MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes.
    pub file_size: i64,
    /// Classification tags.
    pub tags: Vec<String>,
    /// How the file was created (uploaded, imported, generated).
    pub source: FileSource,
    /// Account ID of the user who uploaded/created the file.
    pub uploaded_by: Uuid,
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
    pub fn from_model(file: FileModel) -> Self {
        Self {
            id: file.id,
            workspace_id: file.workspace_id,
            display_name: file.display_name,
            original_filename: file.original_filename,
            file_extension: file.file_extension,
            mime_type: file.mime_type,
            file_size: file.file_size_bytes,
            tags: file.tags.into_iter().flatten().collect(),
            source: file.source,
            uploaded_by: file.account_id,
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
