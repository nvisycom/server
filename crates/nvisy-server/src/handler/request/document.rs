//! Document request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Request payload for creating a new document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Q4 Financial Report"
}))]
pub struct CreateDocumentRequest {
    /// Display name of the document.
    #[validate(length(min = 1, max = 255))]
    pub display_name: String,
}

/// Request payload to update a document.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Updated Report Name"
}))]
pub struct UpdateDocumentRequest {
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
}
