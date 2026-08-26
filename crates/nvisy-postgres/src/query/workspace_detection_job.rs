//! Detection-job outbox repository: the write side (insert in the create-detection
//! transaction) and the drainer side (claim a due batch, mark processed, defer or
//! dead-letter a failure).
//!
//! The drainer runs the claim and the subsequent `mark_*`/`defer_*` inside one
//! transaction per batch (see the detection-job drainer), so the `FOR UPDATE SKIP
//! LOCKED` locks are held from claim through completion: no other drainer takes
//! the same rows, and a row's state transition commits atomically with its
//! publication.

use std::future::Future;

use diesel::prelude::*;
use diesel::sql_types::{BigInt, Timestamptz};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceDetectionJob, WorkspaceDetectionJob};
use crate::types::OutboxStatus;
use crate::{PgConnection, PgError, PgResult, schema};

/// Read and write operations on the detection-job outbox.
pub trait DetectionJobOutboxRepository {
    /// Inserts one job outbox row. Called in the same transaction as the
    /// detection it queues, so the two commit atomically.
    fn insert_detection_job(
        &mut self,
        row: NewWorkspaceDetectionJob,
    ) -> impl Future<Output = PgResult<WorkspaceDetectionJob>> + Send;

    /// Claims up to `limit` due pending rows for publication, oldest first.
    ///
    /// Due means unprocessed, not dead-lettered, and past its `next_attempt_at`,
    /// so a row deferred by a backoff is skipped until its time arrives. Locks the
    /// claimed rows with `FOR UPDATE SKIP LOCKED` so concurrent drainers take
    /// disjoint batches without blocking each other; the lock is held for the
    /// caller's transaction. Must run inside that transaction.
    fn claim_detection_job_batch(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<WorkspaceDetectionJob>>> + Send;

    /// Marks a row processed (its job durably published), taking it out of the
    /// pending set. Runs in the drainer's batch transaction.
    fn mark_detection_job_processed(
        &mut self,
        id: Uuid,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Records a failed attempt: increments `attempts` and defers the next attempt
    /// to `now() + backoff_secs` (computed by the database clock, so a drainer's
    /// wall-clock skew cannot mis-schedule it), leaving the row pending for a
    /// later retry. Runs in the drainer's batch transaction.
    fn defer_detection_job_attempt(
        &mut self,
        id: Uuid,
        backoff_secs: i64,
    ) -> impl Future<Output = PgResult<()>> + Send;

    /// Dead-letters a row: increments `attempts` and marks it `Failed`, taking it
    /// out of the pending set so a job that can never publish stops consuming
    /// drain cycles. The row is retained for inspection. Runs in the drainer's
    /// batch transaction.
    fn mark_detection_job_failed(&mut self, id: Uuid) -> impl Future<Output = PgResult<()>> + Send;
}

impl DetectionJobOutboxRepository for PgConnection {
    async fn insert_detection_job(
        &mut self,
        row: NewWorkspaceDetectionJob,
    ) -> PgResult<WorkspaceDetectionJob> {
        use schema::workspace_detection_jobs;

        diesel::insert_into(workspace_detection_jobs::table)
            .values(&row)
            .returning(WorkspaceDetectionJob::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn claim_detection_job_batch(
        &mut self,
        limit: i64,
    ) -> PgResult<Vec<WorkspaceDetectionJob>> {
        use schema::workspace_detection_jobs::{self, dsl};

        workspace_detection_jobs::table
            .filter(dsl::status.eq(OutboxStatus::Pending))
            .filter(dsl::next_attempt_at.le(diesel::dsl::now))
            .order((dsl::next_attempt_at.asc(), dsl::created_at.asc()))
            .limit(limit)
            .select(WorkspaceDetectionJob::as_select())
            .for_update()
            .skip_locked()
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn mark_detection_job_processed(&mut self, id: Uuid) -> PgResult<()> {
        use schema::workspace_detection_jobs::{self, dsl};

        diesel::update(workspace_detection_jobs::table.filter(dsl::id.eq(id)))
            .set((
                dsl::status.eq(OutboxStatus::Processed),
                dsl::attempts.eq(dsl::attempts + 1),
            ))
            .execute(self)
            .await
            .map_err(PgError::from)?;
        Ok(())
    }

    async fn defer_detection_job_attempt(&mut self, id: Uuid, backoff_secs: i64) -> PgResult<()> {
        use schema::workspace_detection_jobs::{self, dsl};

        // The row stays `Pending`; only its attempt count and next-due time move.
        // `now() + (backoff_secs * interval '1 second')` schedules the next attempt
        // by the database clock, not the drainer's.
        let next_attempt_at = diesel::dsl::sql::<Timestamptz>("now() + (")
            .bind::<BigInt, _>(backoff_secs)
            .sql(" * interval '1 second')");
        diesel::update(workspace_detection_jobs::table.filter(dsl::id.eq(id)))
            .set((
                dsl::attempts.eq(dsl::attempts + 1),
                dsl::next_attempt_at.eq(next_attempt_at),
            ))
            .execute(self)
            .await
            .map_err(PgError::from)?;
        Ok(())
    }

    async fn mark_detection_job_failed(&mut self, id: Uuid) -> PgResult<()> {
        use schema::workspace_detection_jobs::{self, dsl};

        diesel::update(workspace_detection_jobs::table.filter(dsl::id.eq(id)))
            .set((
                dsl::status.eq(OutboxStatus::Failed),
                dsl::attempts.eq(dsl::attempts + 1),
            ))
            .execute(self)
            .await
            .map_err(PgError::from)?;
        Ok(())
    }
}
