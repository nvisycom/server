//! Document version response types.

use nvisy_postgres::model::DocumentVersion;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Document version information with full details.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// Version unique ID
    pub version_id: Uuid,
    /// Document ID this version belongs to
    pub document_id: Uuid,
    /// Version number (incremental)
    pub version_number: i32,
    /// Version name/label
    pub version_name: Option<String>,
    /// Display name
    pub display_name: String,
    /// Description of changes
    pub description: Option<String>,
    /// File extension
    pub file_extension: String,
    /// File size in bytes
    pub file_size: i64,
    /// Whether this is the current/active version
    pub is_current: bool,
    /// Whether this version is published
    pub is_published: bool,
    /// Account ID that created this version
    pub created_by: Uuid,
    /// Creation timestamp
    pub created_at: OffsetDateTime,
    /// Last modification timestamp
    pub updated_at: OffsetDateTime,
}

impl From<DocumentVersion> for Version {
    fn from(version: DocumentVersion) -> Self {
        Self {
            version_id: version.id,
            document_id: version.document_id,
            version_number: version.version_number,
            version_name: None, // Would need to be added to model
            display_name: version.display_name,
            description: None, // Would need to be added to model
            file_extension: version.file_extension,
            file_size: version.file_size_bytes,
            is_current: true,   // Default assumption
            is_published: true, // Default assumption
            created_by: version.account_id,
            created_at: version.created_at,
            updated_at: version.updated_at,
        }
    }
}

/// Response for document versions.
pub type Versions = Vec<Version>;
