//! Document service inputs.

use std::collections::HashSet;

use nvisy_postgres::model::UpdateWorkspaceDocument as UpdateDocumentModel;
use nvisy_postgres::types::DocumentFilter;

use crate::service::{EngineService, UnknownFormatToken};

/// Filters for listing documents, in engine-native tokens.
///
/// The format and modality tokens are resolved to concrete file extensions
/// against the engine's codec registry at query time.
#[derive(Default)]
pub struct ListDocumentsInput {
    /// Search term over document names; an empty term imposes no constraint.
    pub search: Option<String>,
    /// File-extension filter tokens (each expands to its format's full set).
    pub formats: Option<Vec<String>>,
    /// Modality filter tokens (`text`, `tabular`, `image`, `audio`).
    pub modality: Option<Vec<String>>,
    /// Exact content SHA-256 to match, as raw bytes.
    pub hash: Option<Vec<u8>>,
}

impl ListDocumentsInput {
    /// Resolves the format and modality tokens to file extensions against the
    /// engine's codec registry, producing the DB filter.
    ///
    /// `formats` and `modality` are separate facets combined with AND: when both
    /// are given, only documents whose extension is in both sets match (their
    /// intersection). A facet that is absent imposes no constraint.
    ///
    /// # Errors
    ///
    /// - [`UnknownFormatToken`] if a `formats` or `modality` token matches no
    ///   known file extension or modality in the engine's codec registry.
    pub fn to_filter(self, engine: &EngineService) -> Result<DocumentFilter, UnknownFormatToken> {
        let formats = self
            .formats
            .map(|t| engine.resolve_extensions(&t))
            .transpose()?;
        let modality = self
            .modality
            .map(|t| engine.resolve_modalities(&t))
            .transpose()?;
        let search = self.search.filter(|s| !s.is_empty());

        Ok(DocumentFilter {
            search,
            extensions: intersect_facets(formats, modality),
            hash: self.hash,
        })
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
            let keep: HashSet<&String> = b.iter().collect();
            Some(a.into_iter().filter(|ext| keep.contains(ext)).collect())
        }
        (only @ Some(_), None) | (None, only @ Some(_)) => only,
        (None, None) => None,
    }
}

/// Input for updating a document's metadata.
pub struct UpdateDocumentInput {
    /// New display name for the document.
    pub display_name: Option<String>,
    /// Updated metadata blob.
    pub metadata: Option<serde_json::Value>,
}

impl UpdateDocumentInput {
    /// Builds the update model, leaving unset columns unchanged.
    pub fn into_model(self) -> UpdateDocumentModel {
        UpdateDocumentModel {
            display_name: self.display_name,
            metadata: self.metadata,
            ..Default::default()
        }
    }
}
