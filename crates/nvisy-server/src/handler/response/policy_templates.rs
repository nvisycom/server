//! Policy template response types.
//!
//! The full template detail is the runtime's own [`Template`] (serialized
//! directly, including its `policy` body). Listings use a lighter summary that
//! omits the body.

use nvisy_template::Template;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Lightweight template view for catalog listings.
///
/// Omits the policy body so a listing costs nothing to render; the full
/// [`Template`] is returned by the single-template endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyTemplateSummary {
    /// Machine identifier, stable across versions.
    pub id: String,
    /// Human-readable template name.
    pub name: String,
    /// Semver version of this template.
    pub version: String,
    /// Date the regulatory text this template encodes became effective.
    pub effective_date: String,
    /// Longer description for reviewers, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl PolicyTemplateSummary {
    /// Projects a runtime [`Template`] to its summary (drops the policy body).
    pub fn from_template(template: &Template) -> Self {
        Self {
            id: template.id.to_string(),
            name: template.name.to_string(),
            version: template.version.to_string(),
            effective_date: template.effective_date.to_string(),
            description: template.description.as_ref().map(ToString::to_string),
        }
    }
}
