//! Document version response types.

use nvisy_postgres::model::DocumentVersion;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Document version information.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// Version unique ID
    pub version_id: Uuid,
    /// Version number (incremental)
    pub version_number: i32,
    /// Display name
    pub display_name: String,
    /// File extension
    pub file_extension: String,
    /// File size in bytes
    pub file_size: i64,
    /// Processing credits used
    pub processing_credits: i32,
    /// Processing duration in milliseconds
    pub processing_duration: i32,
    /// Creation timestamp
    pub created_at: OffsetDateTime,
}

impl From<DocumentVersion> for Version {
    fn from(version: DocumentVersion) -> Self {
        Self {
            version_id: version.id,
            version_number: version.version_number,
            display_name: version.display_name,
            file_extension: version.file_extension,
            file_size: version.file_size_bytes,
            processing_credits: version.processing_credits,
            processing_duration: version.processing_duration,
            created_at: version.created_at,
        }
    }
}

/// Response for document versions.
pub type Versions = Vec<Version>;
