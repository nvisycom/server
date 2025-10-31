//! Document version model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::document_versions;

/// Document version model representing a specific version of a document file.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = document_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DocumentVersion {
    /// Unique version identifier
    pub id: Uuid,
    /// Reference to the document this version belongs to
    pub document_id: Uuid,
    /// Reference to the account that owns this version
    pub account_id: Uuid,
    /// Version number (incremental)
    pub version_number: i32,
    /// Display name for this version
    pub display_name: String,
    /// File extension (without the dot)
    pub file_extension: String,
    /// Processing credits used
    pub processing_credits: i32,
    /// Processing duration in milliseconds
    pub processing_duration: i32,
    /// Number of API calls made during processing
    pub api_calls_made: i32,
    /// File size in bytes
    pub file_size_bytes: i64,
    /// SHA-256 hash of the file
    pub file_hash_sha256: Vec<u8>,
    /// Storage path or identifier for the version file
    pub storage_path: String,
    /// Storage bucket location
    pub storage_bucket: String,
    /// Processing results and extracted data (JSON)
    pub results: serde_json::Value,
    /// Version metadata (JSON)
    pub metadata: serde_json::Value,
    /// Timestamp when the version was created
    pub created_at: OffsetDateTime,
    /// Timestamp when the version was last updated
    pub updated_at: OffsetDateTime,
    /// Timestamp when the version was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
    /// Retention period in seconds
    pub keep_for_sec: i32,
    /// Timestamp for automatic deletion (cleanup)
    pub auto_delete_at: Option<OffsetDateTime>,
}

/// Data for creating a new document version.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = document_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocumentVersion {
    /// Document ID
    pub document_id: Uuid,
    /// Account ID
    pub account_id: Uuid,
    /// Version number
    pub version_number: i32,
    /// Display name
    pub display_name: Option<String>,
    /// File extension
    pub file_extension: Option<String>,
    /// Processing credits
    pub processing_credits: Option<i32>,
    /// Processing duration
    pub processing_duration: Option<i32>,
    /// API calls made
    pub api_calls_made: Option<i32>,
    /// File size in bytes
    pub file_size_bytes: Option<i64>,
    /// File hash
    pub file_hash_sha256: Vec<u8>,
    /// Storage path
    pub storage_path: String,
    /// Storage bucket
    pub storage_bucket: Option<String>,
    /// Processing results
    pub results: Option<serde_json::Value>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Keep for seconds
    pub keep_for_sec: Option<i32>,
    /// Auto delete timestamp
    pub auto_delete_at: Option<OffsetDateTime>,
}

/// Data for updating a document version.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = document_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateDocumentVersion {
    /// Display name
    pub display_name: Option<String>,
    /// Processing results
    pub results: Option<serde_json::Value>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Auto delete timestamp
    pub auto_delete_at: Option<OffsetDateTime>,
}

impl DocumentVersion {
    /// Returns whether this is the first version of a document.
    pub fn is_first_version(&self) -> bool {
        self.version_number == 1
    }

    /// Returns whether this version is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether this version is scheduled for automatic deletion.
    pub fn is_scheduled_for_deletion(&self) -> bool {
        self.auto_delete_at.is_some()
    }

    /// Returns whether this version should be automatically deleted now.
    pub fn should_be_deleted(&self) -> bool {
        if let Some(delete_time) = self.auto_delete_at {
            OffsetDateTime::now_utc() >= delete_time
        } else {
            false
        }
    }

    /// Returns the time remaining until automatic deletion.
    pub fn time_until_deletion(&self) -> Option<time::Duration> {
        if let Some(delete_time) = self.auto_delete_at {
            let now = OffsetDateTime::now_utc();
            if delete_time > now {
                Some(delete_time - now)
            } else {
                None // Already past deletion time
            }
        } else {
            None // No deletion scheduled
        }
    }

    /// Returns whether the version is an image based on file extension.
    pub fn is_image(&self) -> bool {
        matches!(
            self.file_extension.to_lowercase().as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg"
        )
    }

    /// Returns whether the version is a document based on file extension.
    pub fn is_document(&self) -> bool {
        matches!(
            self.file_extension.to_lowercase().as_str(),
            "pdf" | "doc" | "docx" | "txt" | "html" | "md" | "rtf"
        )
    }

    /// Returns whether the version is a video based on file extension.
    pub fn is_video(&self) -> bool {
        matches!(
            self.file_extension.to_lowercase().as_str(),
            "mp4" | "avi" | "mov" | "wmv" | "flv" | "mkv" | "webm"
        )
    }

    /// Returns whether the version is audio based on file extension.
    pub fn is_audio(&self) -> bool {
        matches!(
            self.file_extension.to_lowercase().as_str(),
            "mp3" | "wav" | "ogg" | "m4a" | "flac" | "aac"
        )
    }

    /// Returns whether the version was created recently (within last 24 hours).
    pub fn is_recently_created(&self) -> bool {
        let now = time::OffsetDateTime::now_utc();
        let duration = now - self.created_at;
        duration.whole_days() < 1
    }

    /// Returns whether the version was updated recently (within last hour).
    pub fn is_recently_updated(&self) -> bool {
        let now = time::OffsetDateTime::now_utc();
        let duration = now - self.updated_at;
        duration.whole_hours() < 1
    }

    /// Returns the file extension with a dot prefix.
    pub fn file_extension_with_dot(&self) -> String {
        if self.file_extension.is_empty() {
            String::new()
        } else {
            format!(".{}", self.file_extension)
        }
    }

    /// Returns whether the version has processing results.
    pub fn has_processing_results(&self) -> bool {
        !self.results.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the version has metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the version is large (over 10MB).
    pub fn is_large_file(&self) -> bool {
        self.file_size_bytes > 10_000_000 // 10MB
    }

    /// Returns whether the version has a valid file hash.
    pub fn has_valid_hash(&self) -> bool {
        !self.file_hash_sha256.is_empty()
    }

    /// Returns a shortened version of the file hash for display.
    pub fn hash_short(&self) -> String {
        if self.file_hash_sha256.len() >= 4 {
            format!(
                "{:02x}{:02x}{:02x}{:02x}...",
                self.file_hash_sha256[0],
                self.file_hash_sha256[1],
                self.file_hash_sha256[2],
                self.file_hash_sha256[3]
            )
        } else {
            String::new()
        }
    }

    /// Returns the age of the version since creation.
    pub fn age(&self) -> time::Duration {
        time::OffsetDateTime::now_utc() - self.created_at
    }

    /// Returns whether this version needs immediate attention (scheduled for deletion soon).
    pub fn needs_attention(&self) -> bool {
        if let Some(remaining) = self.time_until_deletion() {
            remaining.whole_days() < 1 // Less than 1 day until deletion
        } else {
            false
        }
    }

    /// Returns the formatted version number for display.
    pub fn version_display(&self) -> String {
        format!("v{}", self.version_number)
    }

    /// Returns whether this version can be compared with another version.
    pub fn can_compare_with(&self, other: &DocumentVersion) -> bool {
        self.document_id == other.document_id && !self.is_deleted() && !other.is_deleted()
    }

    /// Returns whether this version is newer than another version.
    pub fn is_newer_than(&self, other: &DocumentVersion) -> bool {
        self.document_id == other.document_id && self.version_number > other.version_number
    }

    /// Returns whether this version is older than another version.
    pub fn is_older_than(&self, other: &DocumentVersion) -> bool {
        self.document_id == other.document_id && self.version_number < other.version_number
    }
}
