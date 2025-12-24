//! Document comment request types.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
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

/// Request payload to update a document comment.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDocumentComment {
    /// Updated comment content.
    #[validate(length(min = 1, max = 10000))]
    pub content: Option<String>,
}
