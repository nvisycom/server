//! Document request types.

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
