//! Document file response types.

use jiff::Timestamp;
use nvisy_postgres::model::DocumentFile;
use nvisy_postgres::types::{ContentSegmentation, CursorPage, ProcessingStatus};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Knowledge-related fields for file responses.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileKnowledge {
    /// Whether the file is indexed for knowledge extraction.
    pub is_indexed: bool,

    /// Content segmentation strategy.
    pub content_segmentation: ContentSegmentation,

    /// Whether visual elements are supported.
    pub visual_support: bool,
}

/// Represents an uploaded file.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct File {
    /// Unique file identifier.
    pub file_id: Uuid,
    /// Display name.
    pub display_name: String,
    /// File size in bytes.
    pub file_size: i64,
    /// Processing status.
    pub status: ProcessingStatus,
    /// Processing priority (1-10).
    pub processing_priority: i32,
    /// Classification tags.
    pub tags: Vec<String>,
    /// Knowledge extraction settings.
    pub file_knowledge: FileKnowledge,
    /// Creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

impl File {
    /// Creates a File response from a database model.
    pub fn from_model(file: DocumentFile) -> Self {
        Self {
            file_id: file.id,
            display_name: file.display_name,
            file_size: file.file_size_bytes,
            status: file.processing_status,
            processing_priority: file.processing_priority,
            tags: file.tags.into_iter().flatten().collect(),
            file_knowledge: FileKnowledge {
                is_indexed: file.is_indexed,
                content_segmentation: file.content_segmentation,
                visual_support: file.visual_support,
            },
            created_at: file.created_at.into(),
            updated_at: file.updated_at.into(),
        }
    }

    /// Creates a list of File responses from database models.
    pub fn from_models(models: Vec<DocumentFile>) -> Vec<Self> {
        models.into_iter().map(Self::from_model).collect()
    }
}

/// Response for file uploads (simple list without pagination).
pub type Files = Vec<File>;

/// Paginated response for file listing.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilesPage {
    /// The files in this page.
    pub items: Vec<File>,
    /// Total count of files matching the query (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    /// Whether there are more files after this page.
    pub has_more: bool,
    /// Cursor to fetch the next page, if there are more files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl FilesPage {
    /// Creates a FilesPage from a CursorPage of DocumentFile models.
    pub fn from_cursor_page(page: CursorPage<DocumentFile>) -> Self {
        Self {
            items: page.items.into_iter().map(File::from_model).collect(),
            total: page.total,
            has_more: page.has_more,
            next_cursor: page.next_cursor,
        }
    }
}
