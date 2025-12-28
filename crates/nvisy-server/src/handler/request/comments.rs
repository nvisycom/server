//! Document comment request types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new document comment.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocumentComment {
    /// Comment text content.
    #[validate(length(min = 1, max = 10000))]
    pub content: String,
    /// Parent comment ID for threaded replies.
    #[serde(default)]
    pub parent_comment_id: Option<Uuid>,
    /// Account being replied to (@mention).
    #[serde(default)]
    pub reply_to_account_id: Option<Uuid>,
}

impl CreateDocumentComment {
    /// Converts to database model.
    pub fn into_model(
        self,
        account_id: Uuid,
        file_id: Uuid,
    ) -> nvisy_postgres::model::NewDocumentComment {
        nvisy_postgres::model::NewDocumentComment {
            file_id,
            account_id,
            parent_comment_id: self.parent_comment_id,
            reply_to_account_id: self.reply_to_account_id,
            content: self.content,
            ..Default::default()
        }
    }
}

/// Request payload to update a document comment.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDocumentComment {
    /// Updated comment content.
    #[validate(length(min = 1, max = 10000))]
    pub content: Option<String>,
}

impl UpdateDocumentComment {
    /// Converts to database model.
    pub fn into_model(self) -> nvisy_postgres::model::UpdateDocumentComment {
        nvisy_postgres::model::UpdateDocumentComment {
            content: self.content,
            ..Default::default()
        }
    }
}
