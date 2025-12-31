//! Document comment response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a document comment.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    /// ID of the comment.
    pub comment_id: Uuid,
    /// ID of the file this comment belongs to.
    pub file_id: Uuid,
    /// ID of the account that created the comment.
    pub account_id: Uuid,
    /// Parent comment ID for threaded replies.
    pub parent_comment_id: Option<Uuid>,
    /// Account being replied to (@mention).
    pub reply_to_account_id: Option<Uuid>,
    /// Comment text content.
    pub content: Option<String>,
    /// Timestamp when the comment was created.
    pub created_at: Timestamp,
    /// Timestamp when the comment was last updated.
    pub updated_at: Timestamp,
}

impl Comment {
    /// Creates a Comment response from a database model.
    pub fn from_model(comment: model::DocumentComment) -> Self {
        Self {
            comment_id: comment.id,
            file_id: comment.file_id,
            account_id: comment.account_id,
            parent_comment_id: comment.parent_comment_id,
            reply_to_account_id: comment.reply_to_account_id,
            content: comment.get_content(),
            created_at: comment.created_at.into(),
            updated_at: comment.updated_at.into(),
        }
    }

    /// Creates a list of Comment responses from database models.
    pub fn from_models(models: Vec<model::DocumentComment>) -> Vec<Self> {
        models.into_iter().map(Self::from_model).collect()
    }
}

/// Response for listing comments.
pub type Comments = Vec<Comment>;
