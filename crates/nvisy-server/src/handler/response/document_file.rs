//! Document file response types.

use nvisy_postgres::types::ProcessingStatus;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents an uploaded file.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// Unique file identifier
    pub file_id: Uuid,
    /// Display name
    pub display_name: String,
    /// File size in bytes
    pub file_size: i64,
    /// Processing status
    pub status: ProcessingStatus,
    /// Processing priority (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_priority: Option<i32>,
    /// Update timestamp (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<OffsetDateTime>,
}

/// Response for file uploads.
pub type Files = Vec<File>;
