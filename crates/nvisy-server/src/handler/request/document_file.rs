//! Document file request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Upload mode for file uploads.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum UploadMode {
    /// Create a single document with all uploaded files
    Single,
    /// Create individual documents for each uploaded file
    #[default]
    Multiple,
}

/// Request to update file metadata.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "renamed-document.pdf",
    "processingPriority": 10
}))]
pub struct UpdateFile {
    /// New display name for the file
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// New processing priority
    pub processing_priority: Option<i32>,
}
