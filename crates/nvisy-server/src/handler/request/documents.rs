//! Document request types.

use jiff::Timestamp;
use nvisy_postgres::model::{NewDocument, UpdateDocument as UpdateDocumentModel};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::validations::is_alphanumeric;

/// Request payload for creating a new document.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocument {
    /// Display name of the document.
    #[validate(length(min = 1, max = 255))]
    pub display_name: String,
    /// Description of the document.
    #[validate(length(max = 200))]
    pub description: Option<String>,
    /// Tags for document classification.
    #[validate(length(max = 20))]
    pub tags: Option<Vec<String>>,
    /// Document category.
    #[validate(length(max = 50))]
    pub category: Option<String>,
    /// Optional expiration date.
    pub expires_at: Option<Timestamp>,
    /// Whether the document is private.
    pub is_private: Option<bool>,
    /// Whether approval is required.
    pub requires_approval: Option<bool>,
}

impl CreateDocument {
    /// Converts this request into a database model.
    pub fn into_model(self, workspace_id: Uuid, account_id: Uuid) -> NewDocument {
        NewDocument {
            workspace_id,
            account_id,
            display_name: Some(self.display_name),
            description: self.description,
            tags: self.tags.map(|t| t.into_iter().map(Some).collect()),
            ..Default::default()
        }
    }
}

/// Request payload for updating a document.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDocument {
    /// Updated display name.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Updated description.
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    /// Updated tags (must be alphanumeric).
    #[validate(length(min = 1, max = 20))]
    #[validate(custom(function = "is_alphanumeric"))]
    pub tags: Option<Vec<String>>,
    /// Updated category.
    #[validate(length(max = 50))]
    pub category: Option<String>,
    /// Updated expiration date.
    pub expires_at: Option<Timestamp>,
    /// Updated private status.
    pub is_private: Option<bool>,
    /// Updated approval requirement.
    pub requires_approval: Option<bool>,
}

impl UpdateDocument {
    /// Converts this request into a database model.
    pub fn into_model(self) -> UpdateDocumentModel {
        UpdateDocumentModel {
            display_name: self.display_name,
            description: self.description.map(Some),
            tags: self.tags.map(|t| t.into_iter().map(Some).collect()),
            ..Default::default()
        }
    }
}

/// Request payload for document search.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SearchDocuments {
    /// Search query.
    #[validate(length(min = 1, max = 1000))]
    pub query: Option<String>,

    /// Filter by tags.
    #[validate(length(max = 10))]
    pub tags: Option<Vec<String>>,

    /// Filter by categories.
    #[validate(length(max = 5))]
    pub categories: Option<Vec<String>>,

    /// Filter by priority.
    #[validate(length(max = 4))]
    pub priority: Option<Vec<String>>,

    /// Filter from date.
    pub date_from: Option<Timestamp>,

    /// Filter to date.
    pub date_to: Option<Timestamp>,

    /// Include private documents.
    pub include_private: Option<bool>,

    /// Include archived documents.
    pub include_archived: Option<bool>,

    /// Sort field.
    #[validate(length(max = 50))]
    pub sort_by: Option<String>,

    /// Sort direction.
    #[validate(length(max = 10))]
    pub sort_direction: Option<String>,

    /// Maximum results.
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,

    /// Offset for pagination.
    #[validate(range(min = 0, max = 10000))]
    pub offset: Option<u32>,

    /// Search in content.
    pub search_in_content: Option<bool>,

    /// Workspace ID filter.
    pub workspace_id: Option<Uuid>,

    /// Author ID filter.
    pub author_id: Option<Uuid>,
}
