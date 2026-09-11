//! Workspace comment model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value;
use uuid::Uuid;

use crate::schema::workspace_comments;

/// A comment on a file under review.
///
/// A comment is authored by a workspace member, optionally anchored to a location
/// within the file (a modality-tagged [`anchor`](Self::anchor)), optionally a
/// one-level reply to another comment ([`parent_id`](Self::parent_id)), and can
/// be resolved to close its thread.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceComment {
    /// Unique comment identifier.
    pub id: Uuid,
    /// Workspace this comment belongs to (denormalized for fast per-workspace
    /// queries).
    pub workspace_id: Uuid,
    /// File the comment is on.
    pub file_id: Uuid,
    /// Account that wrote the comment.
    pub author_account_id: Uuid,
    /// Parent comment for a one-level reply; `None` for a top-level comment.
    pub parent_id: Option<Uuid>,
    /// The comment text.
    pub body: String,
    /// Optional modality-tagged location within the file the comment is pinned to.
    /// `None` for a file-level comment. Stored as the anchor's typed JSON; the
    /// handler layer decodes it into the typed anchor.
    pub anchor: Option<Value>,
    /// When the thread was resolved; `None` while open.
    pub resolved_at: Option<Timestamp>,
    /// Account that resolved the thread, for the audit trail. `None` if open (or
    /// if that account was since removed).
    pub resolved_by: Option<Uuid>,
    /// When the comment was created.
    pub created_at: Timestamp,
    /// When the comment was last updated.
    pub updated_at: Timestamp,
    /// When the comment was soft-deleted; `None` means live.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new workspace comment.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceComment {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// File ID (required).
    pub file_id: Uuid,
    /// Author account ID (required).
    pub author_account_id: Uuid,
    /// Parent comment for a reply; `None` for a top-level comment.
    pub parent_id: Option<Uuid>,
    /// The comment text (required).
    pub body: String,
    /// Optional anchor JSON.
    pub anchor: Option<Value>,
}

impl NewWorkspaceComment {
    /// A minimal top-level comment on `file_id`, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, file_id: Uuid, author_account_id: Uuid) -> Self {
        Self {
            workspace_id,
            file_id,
            author_account_id,
            body: "A test comment.".to_owned(),
            ..Default::default()
        }
    }
}

/// Data for updating a workspace comment's body.
///
/// Only the body is editable. Resolution and soft-delete are separate repository
/// operations (they set their own audited timestamp columns).
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_comments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceComment {
    /// The new comment text.
    pub body: Option<String>,
}
