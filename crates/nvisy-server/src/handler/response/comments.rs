//! Comment response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceComment as CommentModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AccountRef, Page};
use crate::handler::request::CommentAnchor;

/// Response type for a comment on a file.
///
/// A comment is authored by a workspace member, optionally a one-level reply
/// (`parentId`), optionally pinned to a location within the file (`anchor`), and
/// can be resolved to close its thread.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    /// Unique identifier of the comment.
    pub id: Uuid,
    /// File the comment is on.
    pub file_id: Uuid,
    /// The comment this replies to, for a one-level thread; `None` for a
    /// top-level comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    /// Account that wrote the comment.
    pub author: AccountRef,
    /// The comment text.
    pub body: String,
    /// Location within the file the comment is pinned to, when set. `None` for a
    /// file-level comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<CommentAnchor>,
    /// Whether the thread is resolved.
    pub resolved: bool,
    /// When the thread was resolved, when resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<Timestamp>,
    /// When the comment was created.
    pub created_at: Timestamp,
    /// When the comment was last updated.
    pub updated_at: Timestamp,
}

/// Paginated response for comments.
pub type CommentsPage = Page<Comment>;

impl Comment {
    /// Creates a comment response from the database model and the resolved author
    /// reference.
    pub fn from_model(comment: CommentModel, author: AccountRef) -> Self {
        Self {
            id: comment.id,
            file_id: comment.file_id,
            parent_id: comment.parent_id,
            author,
            body: comment.body,
            // The anchor is stored as its typed JSON; decode it back, treating an
            // undecodable value as no anchor rather than failing the read.
            anchor: comment
                .anchor
                .and_then(|value| serde_json::from_value(value).ok()),
            resolved: comment.resolved_at.is_some(),
            resolved_at: comment.resolved_at.map(Into::into),
            created_at: comment.created_at.into(),
            updated_at: comment.updated_at.into(),
        }
    }
}
