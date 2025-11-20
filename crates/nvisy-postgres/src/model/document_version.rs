//! Document version model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::document_versions;
use crate::types::{HasCreatedAt, HasDeletedAt, HasUpdatedAt};

/// Document version model representing a specific version of a document file.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = document_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DocumentVersion {
    /// Unique version identifier.
    pub id: Uuid,
    /// Reference to the document this version belongs to.
    pub document_id: Uuid,
    /// Reference to the account that owns this version.
    pub account_id: Uuid,
    /// Version number (incremental).
    pub version_number: i32,
    /// Display name for this version.
    pub display_name: String,
    /// File extension (without the dot).
    pub file_extension: String,
    /// Processing credits used.
    pub processing_credits: i32,
    /// Processing duration in milliseconds.
    pub processing_duration: i32,
    /// Number of API calls made during processing.
    pub api_calls_made: i32,
    /// File size in bytes.
    pub file_size_bytes: i64,
    /// SHA-256 hash of the file.
    pub file_hash_sha256: Vec<u8>,
    /// Storage path or identifier for the version file.
    pub storage_path: String,
    /// Storage bucket location.
    pub storage_bucket: String,
    /// Processing results and extracted data (JSON).
    pub results: serde_json::Value,
    /// Version metadata (JSON).
    pub metadata: serde_json::Value,
    /// Timestamp when the version was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the version was last updated.
    pub updated_at: OffsetDateTime,
    /// Timestamp when the version was soft-deleted.
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
    /// Document ID.
    pub document_id: Uuid,
    /// Account ID.
    pub account_id: Uuid,
    /// Version number.
    pub version_number: i32,
    /// Display name.
    pub display_name: Option<String>,
    /// File extension.
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

    /// Returns the SHA-256 hash as a hex string.
    pub fn hash_hex(&self) -> String {
        self.file_hash_sha256
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    /// Returns a shortened version of the hash for display.
    pub fn hash_short(&self) -> String {
        let hex_hash = self.hash_hex();
        if hex_hash.len() > 8 {
            format!("{}...", &hex_hash[..8])
        } else {
            hex_hash
        }
    }

    /// Returns the processing duration in seconds.
    pub fn processing_duration_seconds(&self) -> f64 {
        self.processing_duration as f64 / 1000.0
    }

    /// Returns whether this version has processing results.
    pub fn has_results(&self) -> bool {
        !self.results.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether this version has metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether this is a high-cost version (used many credits).
    pub fn is_high_cost(&self) -> bool {
        self.processing_credits > 100 // Arbitrary threshold
    }

    /// Returns whether this version required many API calls.
    pub fn is_api_intensive(&self) -> bool {
        self.api_calls_made > 50 // Arbitrary threshold
    }

    /// Returns the file extension with a dot prefix.
    pub fn file_extension_with_dot(&self) -> String {
        if self.file_extension.starts_with('.') {
            self.file_extension.clone()
        } else {
            format!(".{}", self.file_extension)
        }
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

    /// Returns whether this is a large file (over 10MB).
    pub fn is_large_file(&self) -> bool {
        self.file_size_bytes > 10_000_000
    }

    /// Returns the processing efficiency (bytes per credit).
    pub fn processing_efficiency(&self) -> Option<f64> {
        if self.processing_credits > 0 {
            Some(self.file_size_bytes as f64 / self.processing_credits as f64)
        } else {
            None
        }
    }

    /// Returns the average processing time per API call (in milliseconds).
    pub fn avg_processing_time_per_call(&self) -> Option<f64> {
        if self.api_calls_made > 0 {
            Some(self.processing_duration as f64 / self.api_calls_made as f64)
        } else {
            None
        }
    }

    /// Returns whether this version can be compared with another version number.
    pub fn is_newer_than(&self, other_version: i32) -> bool {
        self.version_number > other_version
    }

    /// Returns whether this version is older than another version number.
    pub fn is_older_than(&self, other_version: i32) -> bool {
        self.version_number < other_version
    }

    /// Returns the age of this version since creation.
    pub fn age(&self) -> time::Duration {
        OffsetDateTime::now_utc() - self.created_at
    }
}

impl HasCreatedAt for DocumentVersion {
    fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }
}

impl HasUpdatedAt for DocumentVersion {
    fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

impl HasDeletedAt for DocumentVersion {
    fn deleted_at(&self) -> Option<OffsetDateTime> {
        self.deleted_at
    }
}
