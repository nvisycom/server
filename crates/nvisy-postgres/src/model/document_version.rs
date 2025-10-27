//! Document version model for PostgreSQL database operations.

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::document_versions;
use crate::types::FileType;

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
    /// MIME type of the version file
    pub mime_type: String,
    /// File type categorization
    pub file_type: FileType,
    /// Processing credits used
    pub processing_credits: i32,
    /// Processing duration in milliseconds
    pub processing_duration_ms: i32,
    /// Processing cost in USD
    pub processing_cost_usd: Option<BigDecimal>,
    /// Number of API calls made during processing
    pub api_calls_made: i32,
    /// Accuracy score for the processing
    pub accuracy_score: BigDecimal,
    /// Completeness score
    pub completeness_score: BigDecimal,
    /// Confidence score
    pub confidence_score: BigDecimal,
    /// File size in bytes
    pub file_size_bytes: i64,
    /// Storage path or identifier for the version file
    pub storage_path: String,
    /// Storage bucket location
    pub storage_bucket: String,
    /// SHA-256 hash of the file
    pub file_hash_sha256: Vec<u8>,
    /// Whether the file is encrypted
    pub is_encrypted: bool,
    /// Encryption key identifier
    pub encryption_key_id: Option<String>,
    /// Processing results and extracted data (JSON)
    pub processing_results: serde_json::Value,
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
#[derive(Debug, Clone, Insertable)]
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
    pub display_name: String,
    /// File extension
    pub file_extension: String,
    /// MIME type
    pub mime_type: String,
    /// File type
    pub file_type: FileType,
    /// Processing credits
    pub processing_credits: i32,
    /// Processing duration
    pub processing_duration_ms: i32,
    /// Processing cost
    pub processing_cost_usd: Option<BigDecimal>,
    /// API calls made
    pub api_calls_made: i32,
    /// Accuracy score
    pub accuracy_score: BigDecimal,
    /// Completeness score
    pub completeness_score: BigDecimal,
    /// Confidence score
    pub confidence_score: BigDecimal,
    /// File size in bytes
    pub file_size_bytes: i64,
    /// Storage path
    pub storage_path: String,
    /// Storage bucket
    pub storage_bucket: String,
    /// File hash
    pub file_hash_sha256: Vec<u8>,
    /// Is encrypted
    pub is_encrypted: bool,
    /// Encryption key ID
    pub encryption_key_id: Option<String>,
    /// Processing results
    pub processing_results: serde_json::Value,
    /// Metadata
    pub metadata: serde_json::Value,
    /// Keep for seconds
    pub keep_for_sec: i32,
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
    pub processing_results: Option<serde_json::Value>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Auto delete timestamp
    pub auto_delete_at: Option<OffsetDateTime>,
}

impl Default for NewDocumentVersion {
    fn default() -> Self {
        Self {
            document_id: Uuid::new_v4(),
            account_id: Uuid::new_v4(),
            version_number: 1,
            display_name: String::new(),
            file_extension: String::new(),
            mime_type: String::from("application/octet-stream"),
            file_type: FileType::Document,
            processing_credits: 0,
            processing_duration_ms: 0,
            processing_cost_usd: None,
            api_calls_made: 0,
            accuracy_score: BigDecimal::from(0),
            completeness_score: BigDecimal::from(0),
            confidence_score: BigDecimal::from(0),
            file_size_bytes: 0,
            storage_path: String::new(),
            storage_bucket: String::new(),
            file_hash_sha256: Vec::new(),
            is_encrypted: false,
            encryption_key_id: None,
            processing_results: serde_json::Value::Object(serde_json::Map::new()),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            keep_for_sec: 0,
            auto_delete_at: None,
        }
    }
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

    /// Returns the file size in human-readable format.
    pub fn file_size_human(&self) -> String {
        let bytes = if self.file_size_bytes >= 0 {
            self.file_size_bytes as u64
        } else {
            0
        };

        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", bytes, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }

    /// Returns whether the version is an image.
    pub fn is_image(&self) -> bool {
        self.mime_type.starts_with("image/")
    }

    /// Returns whether the version is a document.
    pub fn is_document(&self) -> bool {
        matches!(
            self.mime_type.as_str(),
            "application/pdf"
                | "application/msword"
                | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                | "text/plain"
                | "text/html"
                | "text/markdown"
        )
    }

    /// Returns whether the version is a video.
    pub fn is_video(&self) -> bool {
        self.mime_type.starts_with("video/")
    }

    /// Returns whether the version is audio.
    pub fn is_audio(&self) -> bool {
        self.mime_type.starts_with("audio/")
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
        !self
            .processing_results
            .as_object()
            .is_none_or(|obj| obj.is_empty())
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
