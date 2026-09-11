//! Workspace thread-anchor model: a location within a thread's file the thread is
//! pinned to. A thread may have several; removal is a soft delete so timeline
//! events keep their referent.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value;
use uuid::Uuid;

use crate::schema::workspace_thread_anchors;

/// A location within a thread's file the thread is pinned to.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_thread_anchors)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceThreadAnchor {
    /// Unique anchor identifier.
    pub id: Uuid,
    /// Thread this anchor pins.
    pub thread_id: Uuid,
    /// The modality-tagged location, as typed JSON. Decoded into the typed anchor
    /// by the handler layer.
    pub anchor: Value,
    /// When the anchor was added.
    pub created_at: Timestamp,
    /// When the anchor was removed; `None` means live.
    pub deleted_at: Option<Timestamp>,
}

/// Data for adding an anchor to a thread.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_thread_anchors)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceThreadAnchor {
    /// Thread ID (required).
    pub thread_id: Uuid,
    /// The anchor JSON (required).
    pub anchor: Value,
}
