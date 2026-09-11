//! Workspace thread model: the closable, optionally file-anchored unit of
//! discussion. Its messages are
//! [`WorkspaceThreadComment`](super::WorkspaceThreadComment)s, its pins
//! [`WorkspaceThreadAnchor`](super::WorkspaceThreadAnchor)s, and its lifecycle
//! history [`WorkspaceThreadEvent`](super::WorkspaceThreadEvent)s.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_threads;

/// A discussion thread: the closable, optionally file-anchored unit.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_threads)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceThread {
    /// Unique thread identifier.
    pub id: Uuid,
    /// Workspace this thread belongs to (denormalized).
    pub workspace_id: Uuid,
    /// File the thread is pinned to; `None` for a workspace-level thread.
    pub file_id: Option<Uuid>,
    /// Account that opened the thread.
    pub author_account_id: Uuid,
    /// Optional human-readable title; `None` for an untitled thread.
    pub display_name: Option<String>,
    /// When the thread was closed; `None` while open.
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
    /// File the thread is pinned to; `None` for a workspace-level thread.
    pub file_id: Option<Uuid>,
    /// Opening author account ID (required).
    pub author_account_id: Uuid,
    /// Optional title.
    pub display_name: Option<String>,
}

impl NewWorkspaceThread {
    /// A minimal file-pinned thread opened by `author`, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(workspace_id: Uuid, file_id: Uuid, author_account_id: Uuid) -> Self {
        Self {
            workspace_id,
            file_id: Some(file_id),
            author_account_id,
            display_name: None,
        }
    }
}

/// Data for updating a thread's title.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = workspace_threads)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateWorkspaceThread {
    /// The new title. `Some(None)` clears it, `Some(Some(name))` sets it, `None`
    /// leaves it unchanged.
    pub display_name: Option<Option<String>>,
}
