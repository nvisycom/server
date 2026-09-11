//! Comment response type: one message within a thread.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceThreadComment as CommentModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AccountRef;

/// Response type for a comment: one message within a thread.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    /// Unique identifier of the comment.
    pub id: Uuid,
    /// Thread this message belongs to.
    pub thread_id: Uuid,
    /// Account that wrote the message.
    pub author: AccountRef,
    /// The message text.
    pub body: String,
    /// When the comment was created.
    pub created_at: Timestamp,
    /// When the comment was last updated.
    pub updated_at: Timestamp,
}

impl Comment {
    /// Creates a comment response from the database model and the resolved author
    /// reference.
    pub fn from_model(comment: CommentModel, author: AccountRef) -> Self {
        Self {
            id: comment.id,
            thread_id: comment.thread_id,
            author,
            body: comment.body,
            created_at: comment.created_at.into(),
            updated_at: comment.updated_at.into(),
        }
    }
}
