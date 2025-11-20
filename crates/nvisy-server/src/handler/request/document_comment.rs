//! Document comment request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new document comment.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "content": "This looks great! However, I think we should review the methodology section.",
    "parentCommentId": null,
    "replyToAccountId": null
}))]
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
#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "content": "Updated comment text"
}))]
pub struct UpdateDocumentComment {
    /// Updated comment content.
    #[validate(length(min = 1, max = 10000))]
    pub content: Option<String>,
}
