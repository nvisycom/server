//! Document file request types.

use nvisy_postgres::types::ContentSegmentation;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::service::ArchiveFormat;

/// Request to update file metadata.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFile {
    /// New display name for the file.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,

    /// New processing priority (1-10, higher = more priority).
    #[validate(range(min = 1, max = 10))]
    pub processing_priority: Option<i32>,

    /// Document ID to assign the file to.
    pub document_id: Option<Uuid>,

    /// Whether the file is indexed for knowledge extraction.
    pub is_indexed: Option<bool>,

    /// Content segmentation strategy for knowledge extraction.
    pub content_segmentation: Option<ContentSegmentation>,

    /// Whether visual elements are supported for knowledge extraction.
    pub visual_support: Option<bool>,
}

/// Request to download multiple files.
#[derive(Debug, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloadMultipleFilesRequest {
    /// File IDs to download (1-100 files).
    #[validate(length(min = 1, max = 100))]
    pub file_ids: Vec<Uuid>,
}

/// Request to download files as an archive.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DownloadArchivedFilesRequest {
    /// Archive format (defaults to tar).
    #[serde(default)]
    pub format: ArchiveFormat,

    /// Optional specific file IDs (if None, downloads all project files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_ids: Option<Vec<Uuid>>,
}
