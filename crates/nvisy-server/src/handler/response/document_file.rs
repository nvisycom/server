//! Document file response types.

use nvisy_postgres::types::ProcessingStatus;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Response for a single uploaded file.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UploadedFile {
    /// Unique file identifier
    pub file_id: Uuid,
    /// Display name
    pub display_name: String,
    /// File size in bytes
    pub file_size: i64,
    /// Processing status
    pub status: ProcessingStatus,
}

/// Response returned after successful file upload.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UploadFileResponse {
    /// List of successfully uploaded files
    pub files: Vec<UploadedFile>,
    /// Number of files uploaded
    pub count: usize,
}

/// Response after updating file metadata.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFileResponse {
    /// Updated file information
    pub file_id: Uuid,
    pub display_name: String,
    pub processing_priority: i32,
    pub updated_at: OffsetDateTime,
}
