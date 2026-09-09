//! Workspace file model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_files;
use crate::types::FileKind;

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
    /// File extension (without the dot); Content-Type is derived from it.
    pub file_extension: String,
    /// The file's role (original, redacted, artifact, audit).
    pub file_kind: FileKind,
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
    /// Data-retention expiry (`None` = keep indefinitely).
    pub expires_at: Option<Timestamp>,
    /// When the backing object was reclaimed from the store. `None` on a
    /// soft-deleted row means the purge is still pending (the reaper's to-do).
    pub purged_at: Option<Timestamp>,
}

/// Data for creating a new workspace file.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
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
    /// The file's role.
    pub file_kind: Option<FileKind>,
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
    /// Data-retention expiry (`None` = keep indefinitely).
    pub expires_at: Option<Timestamp>,
}

impl NewWorkspaceFile {
    /// A minimal `original` file for `workspace_id`, for tests.
    ///
    /// Supplies the required storage fields and a valid 32-byte hash; the name,
    /// extension, and kind take their database defaults.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, account_id: Uuid) -> Self {
        let suffix = Uuid::now_v7().simple().to_string();
        Self {
            workspace_id,
            account_id,
            file_size_bytes: 1024,
            file_hash_sha256: vec![0u8; 32],
            storage_path: format!("test/{suffix}"),
            storage_bucket: "test-bucket".to_owned(),
            ..Default::default()
        }
    }
}

/// Data for updating a workspace file.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceFile {
    /// Display name.
    pub display_name: Option<String>,
    /// Parent file ID.
    pub parent_id: Option<Option<Uuid>>,
    /// The file's role.
    pub file_kind: Option<FileKind>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
    /// Soft delete timestamp.
    pub deleted_at: Option<Option<Timestamp>>,
    /// Data-retention expiry.
    pub expires_at: Option<Option<Timestamp>>,
}
