//! Workspace file model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_files;
use crate::types::{FileSource, HasCreatedAt, HasDeletedAt, HasUpdatedAt, RECENTLY_UPLOADED_HOURS};

/// Workspace file model representing a file stored in the system.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceFile {
    /// Unique file identifier.
    pub id: Uuid,
    /// Reference to the workspace this file belongs to.
    pub workspace_id: Uuid,
    /// Reference to the account that owns this file.
    pub account_id: Uuid,
    /// Parent file reference for version chains.
    pub parent_id: Option<Uuid>,
    /// Version number (1 for original, increments for new versions).
    pub version_number: i32,
    /// Human-readable file name for display.
    pub display_name: String,
    /// Original filename when uploaded.
    pub original_filename: String,
    /// File extension (without the dot).
    pub file_extension: String,
    /// MIME type of the file.
    pub mime_type: Option<String>,
    /// Classification tags.
    pub tags: Vec<Option<String>>,
    /// How the file was created (uploaded, imported, generated).
    pub source: FileSource,
    /// File size in bytes.
    pub file_size_bytes: i64,
    /// SHA-256 hash of the file.
    pub file_hash_sha256: Vec<u8>,
    /// Storage path or identifier for the file.
    pub storage_path: String,
    /// Storage bucket name.
    pub storage_bucket: String,
    /// File metadata (JSON).
    pub metadata: serde_json::Value,
    /// Timestamp when the file was uploaded.
    pub created_at: Timestamp,
    /// Timestamp when the file was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the file was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace file.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceFile {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Account ID.
    pub account_id: Uuid,
    /// Parent file ID (for version chains).
    pub parent_id: Option<Uuid>,
    /// Display name.
    pub display_name: Option<String>,
    /// Original filename.
    pub original_filename: Option<String>,
    /// File extension.
    pub file_extension: Option<String>,
    /// MIME type.
    pub mime_type: Option<String>,
    /// Tags.
    pub tags: Option<Vec<Option<String>>>,
    /// How the file was created.
    pub source: Option<FileSource>,
    /// File size in bytes.
    pub file_size_bytes: i64,
    /// SHA-256 hash.
    pub file_hash_sha256: Vec<u8>,
    /// Storage path.
    pub storage_path: String,
    /// Storage bucket.
    pub storage_bucket: String,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a workspace file.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceFile {
    /// Display name.
    pub display_name: Option<String>,
    /// Parent file ID.
    pub parent_id: Option<Option<Uuid>>,
    /// Tags.
    pub tags: Option<Vec<Option<String>>>,
    /// How the file was created.
    pub source: Option<FileSource>,
    /// MIME type.
    pub mime_type: Option<Option<String>>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
}

impl WorkspaceFile {
    /// Returns whether the file was uploaded recently.
    pub fn is_recently_uploaded(&self) -> bool {
        self.was_created_within(jiff::Span::new().hours(RECENTLY_UPLOADED_HOURS))
    }

    /// Returns whether the file is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns the file size in a human-readable format.
    pub fn file_size_human(&self) -> String {
        let bytes = self.file_size_bytes as f64;
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

        if bytes < 1024.0 {
            return format!("{} B", self.file_size_bytes);
        }

        let mut size = bytes;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.1} {}", size, UNITS[unit_index])
    }

    /// Returns the file extension with a dot prefix.
    pub fn file_extension_with_dot(&self) -> String {
        if self.file_extension.starts_with('.') {
            self.file_extension.clone()
        } else {
            format!(".{}", self.file_extension)
        }
    }

    /// Returns whether the file has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the file is a specific type by extension.
    pub fn is_file_type(&self, extension: &str) -> bool {
        self.file_extension.eq_ignore_ascii_case(extension)
    }

    /// Returns whether the file is an image.
    pub fn is_image(&self) -> bool {
        matches!(
            self.file_extension.to_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "bmp"
        )
    }

    /// Returns whether the file is a document.
    pub fn is_document(&self) -> bool {
        matches!(
            self.file_extension.to_lowercase().as_str(),
            "pdf" | "doc" | "docx" | "txt" | "md" | "rtf"
        )
    }

    /// Returns the SHA-256 hash as a hex string.
    pub fn hash_hex(&self) -> String {
        self.file_hash_sha256
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    /// Returns whether this is the original version (version 1).
    pub fn is_original_version(&self) -> bool {
        self.version_number == 1
    }

    /// Returns whether this file is a newer version of another file.
    pub fn is_version_of(&self, other: &WorkspaceFile) -> bool {
        self.parent_id == Some(other.id) && self.version_number > other.version_number
    }
}

impl HasCreatedAt for WorkspaceFile {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspaceFile {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for WorkspaceFile {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
