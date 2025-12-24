//! Document file response types.

use jiff::Timestamp;
use nvisy_postgres::types::{ContentSegmentation, ProcessingStatus};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use uuid::Uuid;

/// Represents an uploaded file.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
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
    pub updated_at: Option<Timestamp>,
}

/// Response for file uploads.
pub type Files = Vec<File>;

/// Knowledge-related fields for document file responses.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentKnowledge {
    /// Whether the file is indexed for knowledge extraction.
    pub is_indexed: bool,

    /// Content segmentation strategy.
    pub content_segmentation: ContentSegmentation,

    /// Whether visual elements are supported.
    pub visual_support: bool,
}
