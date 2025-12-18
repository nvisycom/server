//! Project template request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request body for creating a project template.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectTemplate {
    /// Template name.
    #[schema(example = "Standard Document Template")]
    pub name: String,
    /// Template description.
    #[schema(example = "Standard template for project documentation")]
    pub description: Option<String>,
    /// Template content or configuration as JSON.
    #[schema(example = json!({"sections": ["introduction", "requirements", "implementation"]}))]
    pub content: serde_json::Value,
    /// Template category.
    #[schema(example = "documentation")]
    pub category: Option<String>,
    /// Template tags for organization.
    #[schema(example = json!(["standard", "documentation", "project"]))]
    pub tags: Option<Vec<String>>,
    /// Whether the template is active/available for use.
    #[schema(example = true)]
    pub active: Option<bool>,
    /// Template version.
    #[schema(example = "1.0.0")]
    pub version: Option<String>,
}

/// Request body for updating a project template.
pub type UpdateProjectTemplate = CreateProjectTemplate;
