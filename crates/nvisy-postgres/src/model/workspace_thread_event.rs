//! Workspace thread-event model: an immutable non-message entry in a thread's
//! timeline (opened, closed, reopened, renamed, or an anchor added/removed).

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value;
use uuid::Uuid;

use crate::schema::workspace_thread_events;
use crate::types::ThreadEventKind;

/// An immutable non-message entry in a thread's timeline.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_thread_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceThreadEvent {
    /// Unique event identifier.
    pub id: Uuid,
    /// Workspace this event belongs to (denormalized).
    pub workspace_id: Uuid,
    /// Thread this event belongs to.
    pub thread_id: Uuid,
    /// What happened.
    pub kind: ThreadEventKind,
    /// Account that performed the action; `None` if that account was removed.
    pub actor_account_id: Option<Uuid>,
    /// Event-specific detail (an anchor snapshot for anchor events, the new name
    /// for a rename); `None` for open/close/reopen.
    pub target: Option<Value>,
    /// When the event happened.
    pub created_at: Timestamp,
}

/// Data for recording a new thread timeline event.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_thread_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceThreadEvent {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Thread ID (required).
    pub thread_id: Uuid,
    /// What happened (required).
    pub kind: ThreadEventKind,
    /// Account that performed the action.
    pub actor_account_id: Option<Uuid>,
    /// Event-specific detail; `None` for open/close/reopen.
    pub target: Option<Value>,
}
