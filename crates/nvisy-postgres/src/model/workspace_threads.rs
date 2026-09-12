//! Workspace thread model: a discussion thread, either a workspace-level
//! discussion or a document's review. Its messages are
//! [`WorkspaceThreadComment`](super::WorkspaceThreadComment)s and its lifecycle
//! history [`WorkspaceThreadEvent`](super::WorkspaceThreadEvent)s. A document
//! thread also carries an assignee and a derived [`ReviewStatus`].

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_threads;
use crate::types::ReviewStatus;

/// A discussion thread: a workspace discussion (`document_id` is `None`, open/closed
/// lifecycle) or a document's review (`document_id` is set, carrying an assignee and a
/// derived [`ReviewStatus`]).
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_threads)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceThread {
    /// Unique thread identifier.
    pub id: Uuid,
    /// Workspace this thread belongs to (denormalized).
    pub workspace_id: Uuid,
    /// Document the thread reviews; `None` for a workspace-level thread.
    pub document_id: Option<Uuid>,
    /// Account that opened the thread.
    pub author_account_id: Uuid,
    /// Optional human-readable title; `None` for an untitled thread.
    pub display_name: Option<String>,
    /// Reviewer the document's review is assigned to; `None` when unassigned or on a
    /// workspace thread.
    pub assignee_account_id: Option<Uuid>,
    /// Review state of a document thread, derived from review events; `None` on a
    /// workspace thread.
    pub review_status: Option<ReviewStatus>,
    /// When the thread was closed; `None` while open (workspace threads only).
    pub closed_at: Option<Timestamp>,
    /// Account that closed the thread; `None` if open (or that account was
    /// removed).
    pub closed_by: Option<Uuid>,
    /// When the thread was created.
    pub created_at: Timestamp,
    /// When the thread was last updated.
    pub updated_at: Timestamp,
    /// When the thread was soft-deleted; `None` means live.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new thread.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = workspace_threads)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceThread {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Document the thread reviews; `None` for a workspace-level thread.
    pub document_id: Option<Uuid>,
    /// Opening author account ID (required).
    pub author_account_id: Uuid,
    /// Optional title.
    pub display_name: Option<String>,
    /// Initial review status; must be set for a document thread and `None` for a
    /// workspace thread (the `(document_id IS NULL) = (review_status IS NULL)` check).
    pub review_status: Option<ReviewStatus>,
}

impl NewWorkspaceThread {
    /// A minimal document review thread opened by `author`, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, document_id: Uuid, author_account_id: Uuid) -> Self {
        Self {
            workspace_id,
            document_id: Some(document_id),
            author_account_id,
            display_name: None,
            review_status: Some(ReviewStatus::NeedsReview),
        }
    }
}

/// Data for updating a thread's mutable fields.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_threads)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceThread {
    /// The new title. `Some(None)` clears it, `Some(Some(name))` sets it, `None`
    /// leaves it unchanged.
    pub display_name: Option<Option<String>>,
    /// The new assignee. `Some(None)` clears it, `Some(Some(id))` sets it, `None`
    /// leaves it unchanged.
    pub assignee_account_id: Option<Option<Uuid>>,
    /// The new review status; `None` leaves it unchanged.
    pub review_status: Option<ReviewStatus>,
}
