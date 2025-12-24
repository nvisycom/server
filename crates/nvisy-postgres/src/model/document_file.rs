//! Document file model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::document_files;
use crate::types::constants::file;
use crate::types::{
    ContentSegmentation, HasCreatedAt, HasDeletedAt, HasUpdatedAt, ProcessingStatus, RequireMode,
    VirusScanStatus,
};

/// Document file model representing a file attached to a document.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = document_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DocumentFile {
    /// Unique file identifier.
    pub id: Uuid,
    /// Reference to the project this file belongs to (required).
    pub project_id: Uuid,
    /// Reference to the document this file belongs to (optional).
    pub document_id: Option<Uuid>,
    /// Reference to the account that owns this file.
    pub account_id: Uuid,
    /// Parent file reference for hierarchical relationships.
    pub parent_id: Option<Uuid>,
    /// Whether file content has been indexed for search.
    pub is_indexed: bool,
    /// Human-readable file name for display.
    pub display_name: String,
    /// Original filename when uploaded.
    pub original_filename: String,
    /// File extension (without the dot).
    pub file_extension: String,
    /// Classification tags.
    pub tags: Vec<Option<String>>,
    /// Processing mode requirements.
    pub require_mode: RequireMode,
    /// Processing priority (higher numbers = higher priority).
    pub processing_priority: i32,
    /// Current processing status.
    pub processing_status: ProcessingStatus,
    /// Virus scan status.
    pub virus_scan_status: VirusScanStatus,
    /// Content segmentation strategy.
    pub content_segmentation: ContentSegmentation,
    /// Whether to enable visual content processing.
    pub visual_support: bool,
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
    /// Keep file for this many seconds (NULL for indefinite retention).
    pub keep_for_sec: Option<i32>,
    /// Auto delete timestamp.
    pub auto_delete_at: Option<Timestamp>,
    /// Timestamp when the file was uploaded.
    pub created_at: Timestamp,
    /// Timestamp when the file was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the file was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new document file.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = document_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewDocumentFile {
    /// Project ID (required).
    pub project_id: Uuid,
    /// Document ID (optional).
    pub document_id: Option<Uuid>,
    /// Account ID.
    pub account_id: Uuid,
    /// Parent file ID.
    pub parent_id: Option<Uuid>,
    /// Is indexed flag.
    pub is_indexed: Option<bool>,
    /// Display name.
    pub display_name: Option<String>,
    /// Original filename.
    pub original_filename: Option<String>,
    /// File extension.
    pub file_extension: Option<String>,
    /// Tags
    pub tags: Option<Vec<Option<String>>>,
    /// Require mode
    pub require_mode: Option<RequireMode>,
    /// Processing priority
    pub processing_priority: Option<i32>,
    /// Processing status
    pub processing_status: Option<ProcessingStatus>,
    /// Virus scan status
    pub virus_scan_status: Option<VirusScanStatus>,
    /// Content segmentation
    pub content_segmentation: Option<ContentSegmentation>,
    /// Visual support
    pub visual_support: Option<bool>,
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
    pub keep_for_sec: Option<i32>,
    /// Auto delete at
    pub auto_delete_at: Option<Timestamp>,
}

/// Data for updating a document file.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = document_files)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateDocumentFile {
    // Note: project_id is required and should not be updated after creation
    /// Document ID
    pub document_id: Option<Option<Uuid>>,
    /// Display name
    pub display_name: Option<String>,
    /// Parent file ID
    pub parent_id: Option<Option<Uuid>>,
    /// Is indexed flag
    pub is_indexed: Option<bool>,
    /// Tags
    pub tags: Option<Vec<Option<String>>>,
    /// Require mode
    pub require_mode: Option<RequireMode>,
    /// Processing priority
    pub processing_priority: Option<i32>,
    /// Processing status
    pub processing_status: Option<ProcessingStatus>,
    /// Virus scan status
    pub virus_scan_status: Option<VirusScanStatus>,
    /// Content segmentation
    pub content_segmentation: Option<ContentSegmentation>,
    /// Visual support
    pub visual_support: Option<bool>,
    /// Metadata
    pub metadata: Option<serde_json::Value>,
    /// Soft delete timestamp
    pub deleted_at: Option<Option<Timestamp>>,
}

impl DocumentFile {
    /// Returns whether the file was uploaded recently.
    pub fn is_recently_uploaded(&self) -> bool {
        self.was_created_within(jiff::Span::new().hours(file::RECENTLY_UPLOADED_HOURS))
    }

    /// Returns whether the file is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the file is ready for use.
    pub fn is_ready(&self) -> bool {
        self.processing_status.is_successful() && self.virus_scan_status.is_safe()
    }

    /// Returns whether the file is safe (passed virus scan).
    pub fn is_safe(&self) -> bool {
        matches!(self.virus_scan_status, VirusScanStatus::Clean)
    }

    /// Returns whether the file has failed virus scanning.
    pub fn has_virus(&self) -> bool {
        matches!(self.virus_scan_status, VirusScanStatus::Infected)
    }

    /// Returns whether the file is currently being processed.
    pub fn is_processing(&self) -> bool {
        self.processing_status.is_processing()
    }

    /// Returns whether the file has completed processing.
    pub fn is_processed(&self) -> bool {
        self.processing_status.is_successful()
    }

    /// Returns whether the file processing has failed.
    pub fn has_processing_error(&self) -> bool {
        self.processing_status.is_failed()
    }

    /// Returns whether the file is scheduled for auto-deletion.
    pub fn is_scheduled_for_deletion(&self) -> bool {
        self.auto_delete_at.is_some()
    }

    /// Returns whether the file should be auto-deleted now.
    pub fn should_be_deleted(&self) -> bool {
        if let Some(delete_at) = self.auto_delete_at {
            jiff::Timestamp::now() >= jiff::Timestamp::from(delete_at)
        } else {
            false
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

    /// Returns the time remaining until auto-deletion.
    pub fn time_until_deletion(&self) -> Option<jiff::Span> {
        if let Some(delete_at) = self.auto_delete_at {
            let now = jiff::Timestamp::now();
            let delete_at = jiff::Timestamp::from(delete_at);
            if delete_at > now {
                Some(delete_at - now)
            } else {
                None
            }
        } else {
            None
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

    /// Returns the SHA-256 hash as a hex string.
    pub fn hash_hex(&self) -> String {
        self.file_hash_sha256
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    /// Returns the processing priority level description.
    pub fn priority_description(&self) -> &'static str {
        match self.processing_priority {
            p if p >= 90 => "Critical",
            p if p >= 70 => "High",
            p if p >= 50 => "Medium",
            p if p >= 30 => "Low",
            _ => "Minimal",
        }
    }
}

impl HasCreatedAt for DocumentFile {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for DocumentFile {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for DocumentFile {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}
