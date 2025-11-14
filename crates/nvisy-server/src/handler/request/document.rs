//! Document request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Request payload for creating a new document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Q4 Financial Report",
    "description": "Quarterly financial report for Q4 2024",
    "tags": ["finance", "report", "q4"]
}))]
pub struct CreateDocument {
    /// Display name of the document.
    #[validate(length(min = 1, max = 255))]
    pub display_name: String,
    /// Description of the document.
    #[serde(default)]
    pub description: String,
    /// Tags for document classification.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Request payload to update a document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Updated Report Name",
    "description": "Updated description",
    "tags": ["finance", "updated"]
}))]
pub struct UpdateDocument {
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}
