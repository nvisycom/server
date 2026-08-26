//! Transactional-outbox model for detection jobs.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_detection_jobs;
use crate::types::OutboxStatus;

/// A pending or processed detection-job outbox row: a serialized `DetectionJob`
/// awaiting (or past) publication to the detection work-queue.
///
/// The `job` column is an opaque JSON blob to this layer — a serialized
/// server-side `DetectionJob` — so the ORM stays free of the job vocabulary; the
/// drainer decodes it and publishes it.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = workspace_detection_jobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceDetectionJob {
    /// Unique outbox row identifier.
    pub id: Uuid,
    /// The detection this job analyzes.
    pub detection_id: Uuid,
    /// The serialized detection job.
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

/// A new detection-job outbox row, inserted in the same transaction as the
/// detection it queues.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_detection_jobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceDetectionJob {
    /// The detection this job analyzes.
    pub detection_id: Uuid,
    /// The serialized detection job.
    pub job: serde_json::Value,
}
