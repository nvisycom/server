//! Project template request types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Request body for creating a project template.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTemplate {
    /// Template name.
    pub name: String,
    /// Template description.
    pub description: Option<String>,
    /// Template content or configuration as JSON.
    pub content: serde_json::Value,
    /// Template category.
    pub category: Option<String>,
    /// Template tags for organization.
    pub tags: Option<Vec<String>>,
    /// Whether the template is active/available for use.
    pub active: Option<bool>,
    /// Template version.
    pub version: Option<String>,
}

/// Request body for updating a project template.
pub type UpdateTemplate = CreateTemplate;
