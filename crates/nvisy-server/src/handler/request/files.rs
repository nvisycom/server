//! File request types.

use std::borrow::Cow;
use std::collections::BTreeSet;

use derive_more::{AsRef, Into};
use elide_pipeline::FormatRegistry;
use nvisy_postgres::model::UpdateWorkspaceFile as UpdateFileModel;
use nvisy_postgres::types::FileFilter;
use schemars::{JsonSchema, Schema, SchemaGenerator};
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
    /// Updated metadata.
    pub metadata: Option<serde_json::Value>,
}

impl UpdateFile {
    pub fn into_model(self) -> UpdateFileModel {
        UpdateFileModel {
            display_name: self.display_name,
            metadata: self.metadata,
            ..Default::default()
        }
    }
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

/// Query parameters for listing files.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListFiles {
    /// Search by file name (case-insensitive, partial match).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// Filter by file extension. Each entry expands to its format's full
    /// extension set (so `jpg` also matches `jpeg`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formats: Option<Vec<FormatToken>>,
    /// Filter by modality (`text`, `tabular`, `image`, `audio`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modality: Option<Vec<ModalityToken>>,
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
        let formats = self.formats.as_deref();
        let formats = formats.map(|t| engine.resolve_extensions(t)).transpose()?;

        let modality = self.modality.as_deref();
        let modality = modality.map(|t| engine.resolve_modalities(t)).transpose()?;

        let mut filter = FileFilter::new();
        if let Some(search) = self.search.as_deref().filter(|s| !s.is_empty()) {
            filter = filter.with_search(search.to_owned());
        }
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
