//! File request types.

use nvisy_postgres::model::UpdateWorkspaceFile as UpdateFileModel;
use nvisy_postgres::types::{FileFilter, FileFormat};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Request to update file metadata.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFile {
    /// New display name for the file.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Updated tags.
    pub tags: Option<Vec<String>>,
    /// Updated metadata.
    pub metadata: Option<serde_json::Value>,
}

impl UpdateFile {
    pub fn into_model(self) -> UpdateFileModel {
        UpdateFileModel {
            display_name: self.display_name,
            tags: self.tags.map(|t| t.into_iter().map(Some).collect()),
            metadata: self.metadata,
            ..Default::default()
        }
    }
}

/// Query parameters for listing files.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListFiles {
    /// Search by file name (case-insensitive, partial match).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// Filter by file formats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formats: Option<Vec<FileFormat>>,
}

impl ListFiles {
    /// Converts to filter model.
    pub fn to_filter(&self) -> FileFilter {
        FileFilter {
            search: self.search.clone(),
            formats: self.formats.clone(),
        }
    }
}
