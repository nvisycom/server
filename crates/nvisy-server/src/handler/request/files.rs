//! File request types.

use nvisy_postgres::model::UpdateWorkspaceFile as UpdateFileModel;
use nvisy_postgres::types::FileFilter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::service::{EngineService, UnknownFormatToken};

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
    /// Filter by file extension (`pdf`, `png`). Each entry expands to its
    /// format's full extension set (so `jpg` also matches `jpeg`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formats: Option<Vec<String>>,
    /// Filter by modality (`text`, `tabular`, `image`, `audio`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modality: Option<Vec<String>>,
}

impl ListFiles {
    /// Converts to the DB filter, resolving format and modality tokens to file
    /// extensions against the engine's codec registry.
    ///
    /// `formats` and `modality` are separate facets combined with AND: when both
    /// are given, only files whose extension is in both sets match (their
    /// intersection). A facet that is absent imposes no constraint. Returns
    /// [`UnknownFormatToken`] if a token matches no known extension or modality.
    pub fn to_filter(&self, engine: &EngineService) -> Result<FileFilter, UnknownFormatToken> {
        let mut filter = FileFilter::new();
        if let Some(search) = self.search.clone().filter(|s| !s.is_empty()) {
            filter = filter.with_search(search);
        }

        let formats = self
            .formats
            .as_ref()
            .map(|t| engine.resolve_extensions(t))
            .transpose()?;
        let modality = self
            .modality
            .as_ref()
            .map(|t| engine.resolve_modalities(t))
            .transpose()?;

        if let Some(extensions) = intersect_facets(formats, modality) {
            filter = filter.with_extensions(extensions);
        }

        Ok(filter)
    }
}

/// Combines the two extension facets with AND: the intersection when both are
/// present, either one alone when only one is, or `None` when neither is.
fn intersect_facets(
    formats: Option<Vec<String>>,
    modality: Option<Vec<String>>,
) -> Option<Vec<String>> {
    match (formats, modality) {
        (Some(a), Some(b)) => {
            let set: std::collections::HashSet<&String> = b.iter().collect();
            Some(a.into_iter().filter(|ext| set.contains(ext)).collect())
        }
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}
