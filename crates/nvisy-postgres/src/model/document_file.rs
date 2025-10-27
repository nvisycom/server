//! Document file model for PostgreSQL database operations.

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::document_files;
use crate::types::{FileType, ProcessingStatus, RequireMode, VirusScanStatus};

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
    /// MIME type of the file
    pub mime_type: String,
    /// File type category for processing
    pub file_type: FileType,
    /// Processing mode requirements
    pub require_mode: RequireMode,
    /// Processing priority (higher numbers = higher priority)
    pub processing_priority: i32,
    /// File metadata (JSON)
    pub metadata: serde_json::Value,
    /// File size in bytes
    pub file_size_bytes: i64,
    /// Storage path or identifier for the file
    pub storage_path: String,
    /// Storage bucket name
    pub storage_bucket: String,
    /// SHA-256 hash of the file
    pub file_hash_sha256: Vec<u8>,
    /// Current processing status
    pub processing_status: ProcessingStatus,
    /// Number of processing attempts
    pub processing_attempts: i32,
    /// Processing error message if any
    pub processing_error: Option<String>,
    /// Processing duration in milliseconds
    pub processing_duration_ms: Option<i32>,
    /// Processing quality score
    pub processing_score: BigDecimal,
    /// Completeness score
    pub completeness_score: BigDecimal,
    /// Confidence score
    pub confidence_score: BigDecimal,
    /// Is the file sensitive
    pub is_sensitive: bool,
    /// Is the file encrypted
    pub is_encrypted: bool,
    /// Virus scan status
    pub virus_scan_status: Option<VirusScanStatus>,
    /// Timestamp when the file was uploaded
    pub created_at: OffsetDateTime,
    /// Timestamp when the file was last updated
    pub updated_at: OffsetDateTime,
    /// Timestamp when the file was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
    /// Keep file for this many seconds
    pub keep_for_sec: i32,
    /// Auto delete timestamp
    pub auto_delete_at: Option<OffsetDateTime>,
}

/// Data for creating a new document file.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = document_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocumentFile {
    /// Document ID
    pub document_id: Uuid,
    /// Account ID
    pub account_id: Uuid,
    /// Display name
    pub display_name: String,
    /// Original filename
    pub original_filename: String,
    /// File extension
    pub file_extension: String,
    /// MIME type
    pub mime_type: String,
    /// File type
    pub file_type: FileType,
    /// Require mode
    pub require_mode: RequireMode,
    /// Processing priority
    pub processing_priority: i32,
    /// Metadata
    pub metadata: serde_json::Value,
    /// File size in bytes
    pub file_size_bytes: i64,
    /// Storage path
    pub storage_path: String,
    /// Storage bucket
    pub storage_bucket: String,
    /// SHA-256 hash
    pub file_hash_sha256: Vec<u8>,
    /// Processing status
    pub processing_status: ProcessingStatus,
    /// Processing attempts
    pub processing_attempts: i32,
    /// Processing error
    pub processing_error: Option<String>,
    /// Processing duration in ms
    pub processing_duration_ms: Option<i32>,
    /// Processing score
    pub processing_score: BigDecimal,
    /// Completeness score
    pub completeness_score: BigDecimal,
    /// Confidence score
    pub confidence_score: BigDecimal,
    /// Is sensitive
    pub is_sensitive: bool,
    /// Is encrypted
    pub is_encrypted: bool,
    /// Virus scan status
    pub virus_scan_status: Option<VirusScanStatus>,
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
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Processing status
    pub processing_status: Option<ProcessingStatus>,
    /// Processing attempts
    pub processing_attempts: Option<i32>,
    /// Processing error
    pub processing_error: Option<String>,
    /// Processing duration
    pub processing_duration_ms: Option<i32>,
    /// Processing score
    pub processing_score: Option<BigDecimal>,
    /// Completeness score
    pub completeness_score: Option<BigDecimal>,
    /// Confidence score
    pub confidence_score: Option<BigDecimal>,
    /// Is sensitive
    pub is_sensitive: Option<bool>,
    /// Is encrypted
    pub is_encrypted: Option<bool>,
    /// Virus scan status
    pub virus_scan_status: Option<VirusScanStatus>,
}

impl Default for NewDocumentFile {
    fn default() -> Self {
        Self {
            document_id: Uuid::new_v4(),
            account_id: Uuid::new_v4(),
            display_name: String::new(),
            original_filename: String::new(),
            file_extension: String::new(),
            mime_type: String::from("application/octet-stream"),
            file_type: FileType::Document,
            require_mode: RequireMode::Text,
            processing_priority: 0,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            file_size_bytes: 0,
            storage_path: String::new(),
            storage_bucket: String::new(),
            file_hash_sha256: Vec::new(),
            processing_status: ProcessingStatus::Pending,
            processing_attempts: 0,
            processing_error: None,
            processing_duration_ms: None,
            processing_score: BigDecimal::from(0),
            completeness_score: BigDecimal::from(0),
            confidence_score: BigDecimal::from(0),
            is_sensitive: false,
            is_encrypted: false,
            virus_scan_status: Some(VirusScanStatus::Clean),
            keep_for_sec: 0,
            auto_delete_at: None,
        }
    }
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
        matches!(self.virus_scan_status, Some(VirusScanStatus::Clean))
    }

    /// Returns whether the file is infected with a virus.
    pub fn is_infected(&self) -> bool {
        matches!(self.virus_scan_status, Some(VirusScanStatus::Infected))
    }

    /// Returns whether the file is suspicious.
    pub fn is_suspicious(&self) -> bool {
        matches!(self.virus_scan_status, Some(VirusScanStatus::Suspicious))
    }

    /// Returns whether the virus scan status is unknown.
    pub fn is_virus_scan_unknown(&self) -> bool {
        matches!(
            self.virus_scan_status,
            Some(VirusScanStatus::Unknown) | None
        )
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

    /// Returns the file size in human-readable format.
    pub fn file_size_human(&self) -> String {
        let bytes = self.file_size_bytes.to_string().parse::<u64>().unwrap_or(0);

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

    /// Returns whether the file is an image.
    pub fn is_image(&self) -> bool {
        self.mime_type.starts_with("image/")
    }

    /// Returns whether the file is a document.
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

    /// Returns whether the file is a video.
    pub fn is_video(&self) -> bool {
        self.mime_type.starts_with("video/")
    }

    /// Returns whether the file is audio.
    pub fn is_audio(&self) -> bool {
        self.mime_type.starts_with("audio/")
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

    /// Returns the processing duration if available.
    pub fn processing_duration(&self) -> Option<time::Duration> {
        // Return processing duration from processing_duration_ms field if available
        self.processing_duration_ms
            .map(|ms| time::Duration::milliseconds(ms as i64))
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
