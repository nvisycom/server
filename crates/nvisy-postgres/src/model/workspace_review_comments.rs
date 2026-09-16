//! Workspace review-comment model: one message within a review's discussion.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_review_comments;

/// A comment: one message within a review's discussion.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_review_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceReviewComment {
    /// Unique comment identifier.
    pub id: Uuid,
    /// For a reply, the comment it answers; `None` for an ordinary message.
    pub parent_id: Option<Uuid>,
    /// Workspace this comment belongs to (denormalized).
    pub workspace_id: Uuid,
    /// Review this message belongs to.
    pub review_id: Uuid,
    /// Account that wrote the message.
    pub author_account_id: Uuid,
    /// The message text.
    pub body: String,
    /// When the comment was created.
    pub created_at: Timestamp,
    /// When the comment was last updated.
    pub updated_at: Timestamp,
    /// When the comment was soft-deleted; `None` means live.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new comment (a message in a review).
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_review_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceReviewComment {
    /// For a reply, the comment it answers; `None` for an ordinary comment. The
    /// database's partial unique index on this column makes at most one live reply
    /// exist per parent.
    pub parent_id: Option<Uuid>,
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Review ID (required).
    pub review_id: Uuid,
    /// Author account ID (required).
    pub author_account_id: Uuid,
    /// The message text (required).
    pub body: String,
}

impl NewWorkspaceReviewComment {
    /// A minimal message in `review_id`, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, review_id: Uuid, author_account_id: Uuid) -> Self {
        Self {
            parent_id: None,
            workspace_id,
            review_id,
            author_account_id,
            body: "A test comment.".to_owned(),
        }
    }
}

/// Data for updating a comment's body. Only the body is editable.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_review_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceReviewComment {
    /// The new message text.
    pub body: Option<String>,
}
