//! Document file request types.

use nvisy_postgres::model::UpdateDocumentFile;
use nvisy_postgres::types::{ContentSegmentation, FileFilter, FileFormat, FileSortBy, SortOrder};
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

    /// Knowledge extraction settings update.
    #[serde(flatten)]
    pub knowledge: Option<UpdateFileKnowledge>,
}

impl UpdateFile {
    /// Converts this request into a database model.
    pub fn into_model(self) -> UpdateDocumentFile {
        UpdateDocumentFile {
            display_name: self.display_name,
            processing_priority: self.processing_priority,
            document_id: self.document_id.map(Some),
            is_indexed: self.knowledge.as_ref().and_then(|k| k.is_indexed),
            content_segmentation: self.knowledge.as_ref().and_then(|k| k.content_segmentation),
            visual_support: self.knowledge.as_ref().and_then(|k| k.visual_support),
            ..Default::default()
        }
    }
}

/// Request to update file knowledge extraction settings.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFileKnowledge {
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

    /// Optional specific file IDs (if None, downloads all workspace files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_ids: Option<Vec<Uuid>>,
}

/// Query parameters for listing files.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesQuery {
    /// Filter by file formats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formats: Option<Vec<FileFormat>>,

    /// Sort by field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<FileSortField>,

    /// Sort order (asc or desc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<SortOrder>,
}

/// Fields to sort files by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FileSortField {
    /// Sort by file name.
    Name,
    /// Sort by upload date.
    Date,
    /// Sort by file size.
    Size,
}

impl ListFilesQuery {
    /// Converts to filter model.
    pub fn to_filter(&self) -> FileFilter {
        FileFilter {
            formats: self.formats.clone(),
        }
    }

    /// Converts to sort model.
    pub fn to_sort(&self) -> FileSortBy {
        let order = self.order.unwrap_or_default();
        match self.sort_by {
            Some(FileSortField::Name) => FileSortBy::Name(order),
            Some(FileSortField::Date) | None => FileSortBy::Date(order),
            Some(FileSortField::Size) => FileSortBy::Size(order),
        }
    }
}
