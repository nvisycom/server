//! Document version response types.

use nvisy_postgres::model::DocumentVersion;
use nvisy_postgres::types::FileType;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Document version information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    /// Version unique ID
    pub version_id: Uuid,
    /// Version number (incremental)
    pub version_number: i32,
    /// Display name
    pub display_name: String,
    /// File extension
    pub file_extension: String,
    /// MIME type
    pub mime_type: String,
    /// File type
    pub file_type: FileType,
    /// File size in bytes
    pub file_size: i64,
    /// Processing credits used
    pub processing_credits: i32,
    /// Processing duration in milliseconds
    pub processing_duration_ms: i32,
    /// Creation timestamp
    pub created_at: OffsetDateTime,
}

impl From<DocumentVersion> for VersionInfo {
    fn from(version: DocumentVersion) -> Self {
        Self {
            version_id: version.id,
            version_number: version.version_number,
            display_name: version.display_name,
            file_extension: version.file_extension,
            mime_type: version.mime_type,
            file_type: version.file_type,
            file_size: version.file_size_bytes,
            processing_credits: version.processing_credits,
            processing_duration_ms: version.processing_duration_ms,
            created_at: version.created_at,
        }
    }
}

/// Response containing document versions list.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadAllVersionsResponse {
    /// List of versions
    pub versions: Vec<VersionInfo>,
    /// Total number of versions
    pub total: usize,
    /// Pagination information
    pub page: i64,
    pub per_page: i64,
}
