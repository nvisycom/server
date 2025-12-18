//! Project template response types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents a project template.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplate {
    /// Unique template identifier.
    pub template_id: Uuid,
    /// ID of the project this template belongs to.
    pub project_id: Uuid,
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
    pub active: bool,
    /// Template version.
    pub version: Option<String>,
    /// Number of times this template has been used.
    pub usage_count: i64,
    /// Timestamp when the template was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the template was last updated.
    pub updated_at: OffsetDateTime,
    /// Timestamp when the template was soft-deleted.
    pub deleted_at: Option<OffsetDateTime>,
}

/// Response for listing project templates.
pub type ProjectTemplates = Vec<ProjectTemplate>;

