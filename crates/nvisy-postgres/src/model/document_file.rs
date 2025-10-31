//! Document file model for PostgreSQL database operations.

use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::document_files;
use crate::types::{ProcessingStatus, RequireMode, VirusScanStatus};

/// Document file model representing a file attached to a document.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = document_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DocumentFile {
    /// Unique file identifier
    pub id: Uuid,
    /// Reference to the document this file belongs to
    pub document_id: Uuid,
    /// Reference to the account that owns this file
    pub account_id: Uuid,
    /// Human-readable file name for display
    pub display_name: String,
    /// Original filename when uploaded
    pub original_filename: String,
    /// File extension (without the dot)
    pub file_extension: String,
    /// Processing mode requirements
    pub require_mode: RequireMode,
    /// Processing priority (higher numbers = higher priority)
    pub processing_priority: i32,
    /// Current processing status
    pub processing_status: ProcessingStatus,
    /// Virus scan status
    pub virus_scan_status: VirusScanStatus,
    /// File size in bytes
    pub file_size_bytes: i64,
    /// SHA-256 hash of the file
    pub file_hash_sha256: Vec<u8>,
    /// Storage path or identifier for the file
    pub storage_path: String,
    /// Storage bucket name
    pub storage_bucket: String,
    /// File metadata (JSON)
    pub metadata: serde_json::Value,
    /// Keep file for this many seconds
    pub keep_for_sec: i32,
    /// Auto delete timestamp
    pub auto_delete_at: Option<OffsetDateTime>,
    /// Timestamp when the file was uploaded
    pub created_at: OffsetDateTime,
    /// Timestamp when the file was last updated
    pub updated_at: OffsetDateTime,
    /// Timestamp when the file was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new document file.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = document_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocumentFile {
    /// Document ID
    pub document_id: Uuid,
    /// Account ID
    pub account_id: Uuid,
    /// Display name
    pub display_name: Option<String>,
    /// Original filename
    pub original_filename: Option<String>,
    /// File extension
    pub file_extension: Option<String>,
    /// Require mode
    pub require_mode: Option<RequireMode>,
    /// Processing priority
    pub processing_priority: Option<i32>,
    /// Processing status
    pub processing_status: Option<ProcessingStatus>,
    /// Virus scan status
    pub virus_scan_status: Option<VirusScanStatus>,
    /// File size in bytes
    pub file_size_bytes: Option<i64>,
    /// SHA-256 hash
    pub file_hash_sha256: Vec<u8>,
    /// Storage path
    pub storage_path: String,
    /// Storage bucket
    pub storage_bucket: Option<String>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Keep for seconds
    pub keep_for_sec: i32,
    /// Auto delete at
    pub auto_delete_at: Option<OffsetDateTime>,
}

/// Data for updating a document file.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = document_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateDocumentFile {
    /// Display name
    pub display_name: Option<String>,
    /// Require mode
    pub require_mode: Option<RequireMode>,
    /// Processing priority
    pub processing_priority: Option<i32>,
    /// Processing status
    pub processing_status: Option<ProcessingStatus>,
    /// Virus scan status
    pub virus_scan_status: Option<VirusScanStatus>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
}

impl DocumentFile {
    /// Returns whether the file is currently being processed.
    pub fn is_processing(&self) -> bool {
        self.processing_status.is_processing()
    }

    /// Returns whether the file processing is complete.
    pub fn is_processed(&self) -> bool {
        matches!(
            self.processing_status,
            ProcessingStatus::Completed | ProcessingStatus::Skipped
        )
    }

    /// Returns whether the file processing has failed.
    pub fn has_processing_failed(&self) -> bool {
        self.processing_status.is_failed()
    }

    /// Returns whether the file is pending processing.
    pub fn is_pending_processing(&self) -> bool {
        self.processing_status.is_pending()
    }

    /// Returns whether the file is safe (virus scan passed).
    pub fn is_safe(&self) -> bool {
        self.virus_scan_status.is_safe()
    }

    // Returns whether the file is dangerous and should be blocked.
    pub fn is_unsafe(&self) -> bool {
        self.virus_scan_status.is_unsafe()
    }

    /// Returns whether the virus scan status is unknown or inconclusive.
    pub fn is_conclusive(&self) -> bool {
        self.virus_scan_status.is_conclusive()
    }

    /// Returns whether the file is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the file can be processed.
    pub fn can_be_processed(&self) -> bool {
        self.processing_status.can_be_processed() && self.is_safe()
    }

    /// Returns whether the file processing can be retried.
    pub fn can_be_retried(&self) -> bool {
        self.processing_status.can_be_retried()
    }

    /// Returns whether the file processing can be canceled.
    pub fn can_be_canceled(&self) -> bool {
        self.processing_status.can_be_canceled()
    }

    /// Returns whether the file has a high processing priority.
    pub fn has_high_priority(&self) -> bool {
        self.processing_priority > 5
    }

    /// Returns whether the file was uploaded recently (within last 24 hours).
    pub fn is_recently_uploaded(&self) -> bool {
        let now = time::OffsetDateTime::now_utc();
        let duration = now - self.created_at;
        duration.whole_days() < 1
    }

    /// Returns whether the file was processed recently (within last hour).
    pub fn is_recently_processed(&self) -> bool {
        // Check if processing was completed recently based on updated_at
        if self.processing_status == ProcessingStatus::Completed {
            let now = time::OffsetDateTime::now_utc();
            let duration = now - self.updated_at;
            duration.whole_hours() < 1
        } else {
            false
        }
    }

    /// Returns the file extension with a dot prefix.
    pub fn file_extension_with_dot(&self) -> String {
        if self.file_extension.is_empty() {
            String::new()
        } else {
            format!(".{}", self.file_extension)
        }
    }

    /// Returns whether the file has metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the file is large (over 10MB).
    pub fn is_large_file(&self) -> bool {
        let bytes = self.file_size_bytes.to_string().parse::<u64>().unwrap_or(0);
        bytes > 10_000_000 // 10MB
    }

    /// Returns whether the file checksum is valid (not empty).
    pub fn has_valid_checksum(&self) -> bool {
        !self.file_hash_sha256.is_empty()
    }

    /// Returns a shortened version of the checksum for display.
    pub fn checksum_short(&self) -> String {
        // Convert binary hash to hex string and show first 8 chars
        let hex_hash = self
            .file_hash_sha256
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        if hex_hash.len() > 8 {
            format!("{}...", &hex_hash[..8])
        } else {
            hex_hash
        }
    }

    /// Returns the age of the file since upload.
    pub fn age(&self) -> time::Duration {
        time::OffsetDateTime::now_utc() - self.created_at
    }
}
