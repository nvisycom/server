//! Document request types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::validation::is_alphanumeric;

/// Request payload for creating a new document.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Q4FinancialReport",
    "description": "Quarterly financial report for Q4 2024",
    "tags": ["finance", "report", "q4"]
}))]
pub struct CreateDocument {
    /// Display name of the document.
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Q4FinancialReport", max_length = 255)]
    pub display_name: String,

    /// Description of the document.
    #[serde(default)]
    #[validate(length(max = 200))]
    #[schema(example = "Quarterly financial report for Q4 2024", max_length = 2000)]
    pub description: Option<String>,

    /// Tags for document classification.
    #[serde(default)]
    #[validate(length(max = 20))]
    #[schema(example = json!(["finance", "report", "q4"]))]
    pub tags: Vec<String>,

    /// Document category.
    #[validate(length(max = 50))]
    #[schema(example = "reports")]
    pub category: Option<String>,

    /// Optional expiration date.
    #[schema(example = "2025-12-31T23:59:59Z")]
    pub expires_at: Option<OffsetDateTime>,

    /// Whether the document is private.
    #[schema(example = false)]
    pub is_private: Option<bool>,

    /// Whether approval is required.
    #[schema(example = false)]
    pub requires_approval: Option<bool>,
}

/// Request payload for updating a document.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "UpdatedReportName",
    "description": "Updated description",
    "tags": ["finance", "updated"]

}))]
pub struct UpdateDocument {
    /// Updated display name.
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "UpdatedReportName")]
    pub display_name: Option<String>,

    /// Updated description.
    #[validate(length(max = 2000))]
    #[schema(example = "Updated description")]
    pub description: Option<String>,

    /// Updated tags (must be alphanumeric).
    #[validate(length(min = 1, max = 20))]
    #[validate(custom(function = "is_alphanumeric"))]
    #[schema(example = json!(["finance", "updated"]))]
    pub tags: Option<Vec<String>>,

    /// Updated category.
    #[validate(length(max = 50))]
    #[schema(example = "reports")]
    pub category: Option<String>,

    /// Updated expiration date.
    pub expires_at: Option<OffsetDateTime>,

    /// Updated private status.
    pub is_private: Option<bool>,

    /// Updated approval requirement.
    pub requires_approval: Option<bool>,
}

/// Request payload for document search.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "query": "financial report",
    "tags": ["finance", "report"],
    "limit": 50
}))]
pub struct SearchDocuments {
    /// Search query.
    #[validate(length(min = 1, max = 1000))]
    #[schema(example = "financial report")]
    pub query: Option<String>,

    /// Filter by tags.
    #[validate(length(max = 10))]
    #[schema(example = json!(["finance", "report"]))]
    pub tags: Option<Vec<String>>,

    /// Filter by categories.
    #[validate(length(max = 5))]
    #[schema(example = json!(["reports"]))]
    pub categories: Option<Vec<String>>,

    /// Filter by priority.
    #[validate(length(max = 4))]
    #[schema(example = json!(["high", "urgent"]))]
    pub priority: Option<Vec<String>>,

    /// Filter from date.
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub date_from: Option<OffsetDateTime>,

    /// Filter to date.
    #[schema(example = "2024-12-31T23:59:59Z")]
    pub date_to: Option<OffsetDateTime>,

    /// Include private documents.
    #[schema(example = false)]
    pub include_private: Option<bool>,

    /// Include archived documents.
    #[schema(example = false)]
    pub include_archived: Option<bool>,

    /// Sort field.
    #[validate(length(max = 50))]
    #[schema(example = "created_at")]
    pub sort_by: Option<String>,

    /// Sort direction.
    #[validate(length(max = 10))]
    #[schema(example = "desc")]
    pub sort_direction: Option<String>,

    /// Maximum results.
    #[validate(range(min = 1, max = 100))]
    #[schema(example = 50)]
    pub limit: Option<u32>,

    /// Offset for pagination.
    #[validate(range(min = 0, max = 10000))]
    #[schema(example = 0)]
    pub offset: Option<u32>,

    /// Search in content.
    #[schema(example = true)]
    pub search_in_content: Option<bool>,

    /// Project ID filter.
    pub project_id: Option<Uuid>,

    /// Author ID filter.
    pub author_id: Option<Uuid>,
}
