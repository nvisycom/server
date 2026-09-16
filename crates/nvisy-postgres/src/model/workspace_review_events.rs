//! Workspace review-event model: an immutable non-message entry in a review's
//! timeline (opened, renamed, a detection/redaction linked, the assignee changed,
//! verified, reopened). The reader merges these with the review's comments into one
//! timeline.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use serde_json::Value;
use uuid::Uuid;

use crate::schema::workspace_review_events;
use crate::types::ReviewEventKind;

/// An immutable entry in a review's activity log.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = workspace_review_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceReviewEvent {
    /// Unique event identifier.
    pub id: Uuid,
    /// Workspace this event belongs to (denormalized).
    pub workspace_id: Uuid,
    /// Review this event belongs to.
    pub review_id: Uuid,
    /// What happened.
    pub kind: ReviewEventKind,
    /// Account that performed the action; `None` if that account was removed.
    pub actor_account_id: Option<Uuid>,
    /// Event-specific detail (the linked detection/redaction id, or the assignee);
    /// `None` for events that carry none.
    pub target: Option<Value>,
    /// When the event happened.
    pub created_at: Timestamp,
}

/// Data for recording a new review activity event.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_review_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceReviewEvent {
    /// Workspace ID (required).
    pub workspace_id: Uuid,
    /// Review ID (required).
    pub review_id: Uuid,
    /// What happened (required).
    pub kind: ReviewEventKind,
    /// Account that performed the action.
    pub actor_account_id: Option<Uuid>,
    /// Event-specific detail; `None` when none.
    pub target: Option<Value>,
}
