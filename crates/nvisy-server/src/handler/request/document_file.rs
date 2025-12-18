//! Document file request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid;
use validator::Validate;

/// Request to update file metadata.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "renamed-document.pdf",
    "processingPriority": 10,
    "documentId": "550e8400-e29b-41d4-a716-446655440000"
}))]
pub struct UpdateFile {
    /// New display name for the file
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,

    /// New processing priority (1-10, higher = more priority)
    #[validate(range(min = 1, max = 10))]
    pub processing_priority: Option<i32>,

    /// Document ID to assign the file to
    pub document_id: Option<uuid::Uuid>,
}
