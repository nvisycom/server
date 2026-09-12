//! Document request types.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet};

use derive_more::{AsRef, Into};
use elide_pipeline::FormatRegistry;
use garde::Validate;
use nvisy_postgres::model::UpdateWorkspaceDocument as UpdateDocumentModel;
use nvisy_postgres::types::DocumentFilter;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::handler::utility::DocumentHash;
use crate::service::{EngineService, UnknownFormatToken};

/// Request to update document metadata.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct UpdateWorkspaceDocument {
    /// New display name for the document.
    #[garde(length(chars, min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Updated metadata.
    pub metadata: Option<serde_json::Value>,
}

impl UpdateWorkspaceDocument {
    pub fn into_model(self) -> UpdateDocumentModel {
        UpdateDocumentModel {
            display_name: self.display_name,
            metadata: self.metadata,
            ..Default::default()
        }
    }
}

/// Request to delete several documents in one call.
///
/// The `100`-id cap bounds the work one call fans out into: the resolve query and
/// the delete transaction.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct DeleteWorkspaceDocuments {
    /// Ids of the documents to delete. Ids that are unknown, already deleted, or
    /// in another workspace are skipped rather than failing the request.
    #[garde(length(min = 1, max = 100))]
    pub document_ids: Vec<Uuid>,
}

/// Defines a transparent string newtype whose OpenAPI schema enumerates the
/// values the built-in codec registry supports, so the API advertises exactly
/// which values are accepted (each is validated again at request time).
///
/// The extractor closure maps each registered format to the strings it
/// contributes; the schema `enum` is their sorted, de-duplicated union.
macro_rules! registry_token {
    ($(#[$meta:meta])* $name:ident => $desc:literal, |$format:ident| $values:expr) => {
        $(#[$meta])*
        #[must_use]
        #[derive(Debug, Clone, Serialize, Deserialize, AsRef, Into)]
        #[serde(transparent)]
        #[as_ref(str)]
        pub struct $name(String);

        impl JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
                let values: Vec<String> = FormatRegistry::with_builtin()
                    .iter()
                    .flat_map(|$format| $values)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                schemars::json_schema!({
                    "type": "string",
                    "enum": values,
                    "description": $desc,
                })
            }
        }
    };
}

registry_token!(
    /// A file-extension filter token (`pdf`, `png`).
    FormatToken => "A supported file extension.",
    |format| format.extensions().iter().map(|e| e.as_ref().to_owned())
);

registry_token!(
    /// A modality filter token (`text`, `tabular`, `image`, `audio`).
    ModalityToken => "A supported document modality.",
    |format| std::iter::once(format.modality().to_owned())
);

/// Query parameters for listing documents.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkspaceDocuments {
    /// Search by document name (case-insensitive, partial match).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// Filter by file extension. Each entry expands to its format's full
    /// extension set (so `jpg` also matches `jpeg`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formats: Option<Vec<FormatToken>>,
    /// Filter by modality (`text`, `tabular`, `image`, `audio`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modality: Option<Vec<ModalityToken>>,
    /// Filter to documents whose content is exactly this SHA-256. Lets a client
    /// check whether identical content already exists in the workspace before
    /// uploading it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<DocumentHash>,
}

impl ListWorkspaceDocuments {
    /// Converts to the DB filter, resolving format and modality tokens to file
    /// extensions against the engine's codec registry.
    ///
    /// `formats` and `modality` are separate facets combined with AND: when both
    /// are given, only documents whose extension is in both sets match (their
    /// intersection). A facet that is absent imposes no constraint. Returns
    /// [`UnknownFormatToken`] if a token matches no known extension or modality.
    pub fn to_filter(&self, engine: &EngineService) -> Result<DocumentFilter, UnknownFormatToken> {
        let formats = self.formats.as_deref();
        let formats = formats.map(|t| engine.resolve_extensions(t)).transpose()?;

        let modality = self.modality.as_deref();
        let modality = modality.map(|t| engine.resolve_modalities(t)).transpose()?;

        // An empty search string is not a real search — normalize it to `None`
        // here so the filter always carries a meaningful term.
        let search = self
            .search
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        Ok(DocumentFilter {
            search,
            extensions: intersect_facets(formats, modality),
            hash: self.hash.as_ref().map(DocumentHash::to_bytes),
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
        // Both facets active: keep only extensions in both sets.
        (Some(a), Some(b)) => {
            let keep: HashSet<&String> = b.iter().collect();
            Some(a.into_iter().filter(|ext| keep.contains(ext)).collect())
        }
        // One facet active: it is the constraint on its own.
        (only @ Some(_), None) | (None, only @ Some(_)) => only,
        // Neither: no extension constraint.
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The filter schemas must advertise the registry's supported values as an
    /// OpenAPI `enum`, so the API contract stays in sync with what the
    /// deployment actually accepts.
    #[test]
    fn filter_schemas_enumerate_supported_values() {
        let mut generator = SchemaGenerator::default();

        let formats = FormatToken::json_schema(&mut generator);
        let formats = formats.as_value()["enum"]
            .as_array()
            .expect("format schema has an enum");
        assert!(formats.iter().any(|v| v == "png"));
        assert!(formats.iter().any(|v| v == "txt"));

        let modalities = ModalityToken::json_schema(&mut generator);
        let modalities = modalities.as_value()["enum"]
            .as_array()
            .expect("modality schema has an enum");
        assert!(modalities.iter().any(|v| v == "text"));
        assert!(modalities.iter().any(|v| v == "image"));
    }
}
