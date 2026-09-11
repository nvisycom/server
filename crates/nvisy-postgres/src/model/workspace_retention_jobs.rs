//! Transactional-outbox model for retention-expiry backfill jobs.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::workspace_retention_jobs;
use crate::types::OutboxStatus;

/// A pending or processed retention-backfill outbox row: a scope whose files'
/// `expires_at` must be reprojected after its retention policy changed.
///
/// The row names only the scope (a workspace, and optionally one pipeline within
/// it), never a frozen expiry — the drainer reads the *current* policy at drain
/// time and reprojects from it, so a later policy change simply supersedes.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = workspace_retention_jobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceRetentionJob {
    /// Unique outbox row identifier.
    pub id: Uuid,
    /// The workspace whose files are reprojected.
    pub workspace_id: Uuid,
    /// The pipeline whose produced files are reprojected; `None` for a
    /// workspace-wide backfill (a settings change).
    pub pipeline_id: Option<Uuid>,
    /// Processing state: pending, processed, or failed (dead-lettered).
    pub status: OutboxStatus,
    /// Number of drain attempts made.
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

/// A new retention-backfill outbox row, inserted in the same transaction as the
/// retention-settings or pipeline-override update that triggers it.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = workspace_retention_jobs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewWorkspaceRetentionJob {
    /// The workspace whose files are reprojected.
    pub workspace_id: Uuid,
    /// The pipeline whose produced files are reprojected; `None` reprojects the
    /// whole workspace (a settings change).
    pub pipeline_id: Option<Uuid>,
}

impl NewWorkspaceRetentionJob {
    /// A workspace-wide backfill job (a settings change), reprojecting every file
    /// scope against the workspace baseline.
    pub fn workspace(workspace_id: Uuid) -> Self {
        Self {
            workspace_id,
            pipeline_id: None,
        }
    }

    /// A pipeline-scoped backfill job (an override change), reprojecting only the
    /// files that pipeline produced.
    pub fn pipeline(workspace_id: Uuid, pipeline_id: Uuid) -> Self {
        Self {
            workspace_id,
            pipeline_id: Some(pipeline_id),
        }
    }
}
