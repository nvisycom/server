//! Transactional-outbox model for assistant-reply jobs.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_assistant_jobs;
use crate::types::OutboxStatus;

/// A pending or processed assistant-reply outbox row: a serialized `AssistantJob`
/// awaiting (or past) publication to the assistant work-queue.
///
/// The `job` column is an opaque JSON blob to this layer — a serialized
/// server-side `AssistantJob` — so the ORM stays free of the job vocabulary; the
/// drainer decodes it and publishes it.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = workspace_assistant_jobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceAssistantJob {
    /// Unique outbox row identifier.
    pub id: Uuid,
    /// The comment that triggered this reply.
    pub comment_id: Uuid,
    /// The serialized assistant job.
    pub job: serde_json::Value,
    /// Processing state: pending, processed, or failed (dead-lettered).
    pub status: OutboxStatus,
    /// Number of publish attempts the drainer has made.
    pub attempts: i32,
    /// Earliest time the row may next be claimed; advanced by a backoff after
    /// each failed attempt.
    pub next_attempt_at: Timestamp,
    /// When the job was queued.
    pub created_at: Timestamp,
    /// When a terminal (processed or failed) row was resolved by an operator;
    /// `None` until then. A manual affordance for inspecting the outbox.
    pub resolved_at: Option<Timestamp>,
}

/// A new assistant-reply outbox row, inserted in the same transaction as the
/// comment that triggers it.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_assistant_jobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceAssistantJob {
    /// The comment that triggered this reply.
    pub comment_id: Uuid,
    /// The serialized assistant job.
    pub job: serde_json::Value,
}

impl NewWorkspaceAssistantJob {
    /// A minimal pending outbox row for `comment_id`, with a placeholder job
    /// payload, for tests. The status, attempts, and next-attempt time take their
    /// database defaults, so the row is immediately due.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(comment_id: Uuid) -> Self {
        Self {
            comment_id,
            job: serde_json::json!({ "kind": "test" }),
        }
    }
}
