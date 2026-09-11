//! Retention-backfill outbox repository: the write side (enqueue in the
//! settings/override update transaction), the drainer side (claim a due batch,
//! mark processed, defer or dead-letter a failure), and the bounded, keyset-paged
//! reprojection the worker applies.
//!
//! The drainer runs the claim and the subsequent `mark_*`/`defer_*` inside one
//! transaction per batch, so the `FOR UPDATE SKIP LOCKED` locks are held from
//! claim through completion: no other drainer takes the same rows, and a row's
//! state transition commits atomically with its work.

use std::future::Future;

use diesel::prelude::*;
use diesel::sql_types::{BigInt, Timestamptz};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{NewWorkspaceRetentionJob, WorkspaceRetentionJob};
use crate::types::{FileKind, OutboxStatus, Retention};
use crate::{Error, PgConnection, Result, schema};

/// How many files one reprojection page updates. Bounds the row set a single
/// `UPDATE` locks, so a scope with a large file history is reprojected in bounded
/// steps rather than one unbounded statement.
pub const REPROJECT_PAGE: i64 = 500;

/// Read and write operations on the retention-backfill outbox.
pub trait RetentionJobOutboxRepository {
    /// Enqueues one backfill job. Called in the same transaction as the
    /// settings/override update it follows, so the two commit atomically.
    ///
    /// At most one pending job may exist per scope (a partial unique index), so a
    /// repeated policy change while a job is still pending is a silent no-op: the
    /// outstanding job already reprojects from the latest policy when it drains.
    fn enqueue_retention_job(
        &mut self,
        row: NewWorkspaceRetentionJob,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Claims up to `limit` due pending rows, oldest first.
    ///
    /// Due means unprocessed, not dead-lettered, and past its `next_attempt_at`.
    /// Locks the claimed rows with `FOR UPDATE SKIP LOCKED` so concurrent drainers
    /// take disjoint batches; the lock is held for the caller's transaction. Must
    /// run inside that transaction.
    fn claim_retention_job_batch(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<WorkspaceRetentionJob>>> + Send;

    /// Marks a row processed, taking it out of the pending set. Runs in the
    /// drainer's batch transaction.
    fn mark_retention_job_processed(&mut self, id: Uuid)
    -> impl Future<Output = Result<()>> + Send;

    /// Records a failed attempt: increments `attempts` and defers the next attempt
    /// to `now() + backoff_secs` (by the database clock), leaving the row pending
    /// for a later retry. Runs in the drainer's batch transaction.
    fn defer_retention_job_attempt(
        &mut self,
        id: Uuid,
        backoff_secs: i64,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Dead-letters a row: increments `attempts` and marks it `Failed`, taking it
    /// out of the pending set. The row is retained for inspection. Runs in the
    /// drainer's batch transaction.
    fn mark_retention_job_failed(&mut self, id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Reprojects one page of a scope's files of `kind` to the resolved
    /// `retention`, projecting each file's `expires_at` from its own `created_at`,
    /// and returns the ids updated this page (empty when the scope is drained).
    ///
    /// `pipeline_id` selects a pipeline's produced files (audit blobs via its
    /// detections; redacted and review blobs via those detections' redactions);
    /// `None` selects every file of `kind` in `workspace_id`. Pages by ascending
    /// `id`: pass the last returned id as `after` to advance, `None` to start.
    fn reproject_files_expiry_page(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Option<Uuid>,
        kind: FileKind,
        retention: Retention,
        after: Option<Uuid>,
    ) -> impl Future<Output = Result<Vec<Uuid>>> + Send;
}

impl RetentionJobOutboxRepository for PgConnection {
    async fn enqueue_retention_job(&mut self, row: NewWorkspaceRetentionJob) -> Result<()> {
        use schema::workspace_retention_jobs;

        // A coalesced duplicate (another pending job for this scope) is a no-op:
        // the outstanding job will reproject from the current policy anyway.
        diesel::insert_into(workspace_retention_jobs::table)
            .values(&row)
            .on_conflict_do_nothing()
            .execute(self)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    async fn claim_retention_job_batch(
        &mut self,
        limit: i64,
    ) -> Result<Vec<WorkspaceRetentionJob>> {
        use schema::workspace_retention_jobs::{self, dsl};

        workspace_retention_jobs::table
            .filter(dsl::status.eq(OutboxStatus::Pending))
            .filter(dsl::next_attempt_at.le(diesel::dsl::now))
            .order((dsl::next_attempt_at.asc(), dsl::created_at.asc()))
            .limit(limit)
            .select(WorkspaceRetentionJob::as_select())
            .for_update()
            .skip_locked()
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn mark_retention_job_processed(&mut self, id: Uuid) -> Result<()> {
        use schema::workspace_retention_jobs::{self, dsl};

        diesel::update(workspace_retention_jobs::table.filter(dsl::id.eq(id)))
            .set((
                dsl::status.eq(OutboxStatus::Processed),
                dsl::attempts.eq(dsl::attempts + 1),
            ))
            .execute(self)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    async fn defer_retention_job_attempt(&mut self, id: Uuid, backoff_secs: i64) -> Result<()> {
        use schema::workspace_retention_jobs::{self, dsl};

        // The row stays `Pending`; only its attempt count and next-due time move.
        // `now() + (backoff_secs * interval '1 second')` schedules by the database
        // clock, not the drainer's.
        let next_attempt_at = diesel::dsl::sql::<Timestamptz>("now() + (")
            .bind::<BigInt, _>(backoff_secs)
            .sql(" * interval '1 second')");
        diesel::update(workspace_retention_jobs::table.filter(dsl::id.eq(id)))
            .set((
                dsl::attempts.eq(dsl::attempts + 1),
                dsl::next_attempt_at.eq(next_attempt_at),
            ))
            .execute(self)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    async fn mark_retention_job_failed(&mut self, id: Uuid) -> Result<()> {
        use schema::workspace_retention_jobs::{self, dsl};

        diesel::update(workspace_retention_jobs::table.filter(dsl::id.eq(id)))
            .set((
                dsl::status.eq(OutboxStatus::Failed),
                dsl::attempts.eq(dsl::attempts + 1),
            ))
            .execute(self)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    async fn reproject_files_expiry_page(
        &mut self,
        workspace_id: Uuid,
        pipeline_id: Option<Uuid>,
        kind: FileKind,
        retention: Retention,
        after: Option<Uuid>,
    ) -> Result<Vec<Uuid>> {
        // Collect one page of file ids in the scope, ascending by id after the
        // cursor, so the UPDATE below touches at most `REPROJECT_PAGE` rows.
        let after = after.unwrap_or(Uuid::nil());
        let file_ids = scope_file_ids_page(self, workspace_id, pipeline_id, kind, after).await?;
        if file_ids.is_empty() {
            return Ok(file_ids);
        }

        apply_retention_to_files(self, &file_ids, retention).await?;
        Ok(file_ids)
    }
}

/// One page of the file ids in a scope, for `kind`, with `id > after`, ascending,
/// capped at [`REPROJECT_PAGE`].
///
/// A `pipeline_id` narrows to that pipeline's produced files through the same
/// joins the reaper's accounting uses (audit blobs via detections; redacted and
/// review blobs via redactions of those detections); `None` selects every live
/// file of `kind` in the workspace.
async fn scope_file_ids_page(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    pipeline_id: Option<Uuid>,
    kind: FileKind,
    after: Uuid,
) -> Result<Vec<Uuid>> {
    use schema::workspace_detections::dsl as detections;
    use schema::workspace_files::dsl as files;
    use schema::workspace_redactions::dsl as redactions;
    use schema::{workspace_detections, workspace_files, workspace_redactions};

    // The scope's membership predicate, applied to the one shared page query over
    // `workspace_files`. A whole-workspace job matches every file of `kind`; a
    // pipeline job matches the files it produced — audit blobs via its detections,
    // redacted and review blobs via redactions of those detections — expressed as
    // an `id IN (subquery)` so the outer query stays a plain `workspace_files`
    // scan and every arm boxes to the same type.
    let scope = workspace_files::table.select(files::id).into_boxed();
    let scope = match (pipeline_id, kind) {
        (None, kind) => scope
            .filter(files::workspace_id.eq(workspace_id))
            .filter(files::file_kind.eq(kind)),
        (Some(pipeline_id), FileKind::Audit) => scope.filter(
            files::id.eq_any(
                workspace_detections::table
                    .filter(detections::pipeline_id.eq(pipeline_id))
                    .filter(detections::audit_file_id.is_not_null())
                    .select(detections::audit_file_id.assume_not_null()),
            ),
        ),
        (Some(pipeline_id), FileKind::Redacted) => scope.filter(
            files::id.eq_any(
                workspace_redactions::table
                    .inner_join(workspace_detections::table)
                    .filter(detections::pipeline_id.eq(pipeline_id))
                    .filter(redactions::output_file_id.is_not_null())
                    .select(redactions::output_file_id.assume_not_null()),
            ),
        ),
        (Some(pipeline_id), FileKind::Review) => scope.filter(
            files::id.eq_any(
                workspace_redactions::table
                    .inner_join(workspace_detections::table)
                    .filter(detections::pipeline_id.eq(pipeline_id))
                    .filter(redactions::review_file_id.is_not_null())
                    .select(redactions::review_file_id.assume_not_null()),
            ),
        ),
        // Originals are ingested, not produced; intermediates are not reprojected
        // on this path.
        (Some(_), _) => return Ok(Vec::new()),
    };

    // One shared page: live files in the scope, id-keyset after the cursor,
    // ascending, bounded to `REPROJECT_PAGE`.
    scope
        .filter(files::deleted_at.is_null())
        .filter(files::id.gt(after))
        .order(files::id.asc())
        .limit(REPROJECT_PAGE)
        .load(conn)
        .await
        .map_err(Error::from)
}

/// Sets `expires_at` on the given files from each file's own `created_at` under
/// `retention`: `Persistent` clears it (`NULL`), `Ephemeral` sets it to
/// `created_at` (eligible at once), `Fixed { days }` sets `created_at + days`.
async fn apply_retention_to_files(
    conn: &mut PgConnection,
    file_ids: &[Uuid],
    retention: Retention,
) -> Result<()> {
    use schema::workspace_files::{self, dsl};

    let target = workspace_files::table.filter(dsl::id.eq_any(file_ids.to_vec()));
    match retention {
        Retention::Persistent => {
            diesel::update(target)
                .set(dsl::expires_at.eq(None::<jiff_diesel::Timestamp>))
                .execute(conn)
                .await
                .map_err(Error::from)?;
        }
        Retention::Ephemeral => {
            diesel::update(target)
                .set(dsl::expires_at.eq(dsl::created_at.nullable()))
                .execute(conn)
                .await
                .map_err(Error::from)?;
        }
        Retention::Fixed { days } => {
            // `created_at + (days * interval '1 day')`, computed per row by the
            // database so each file expires relative to its own creation.
            let expires_at = diesel::dsl::sql::<Timestamptz>("created_at + (")
                .bind::<BigInt, _>(i64::from(days))
                .sql(" * interval '1 day')");
            diesel::update(target)
                .set(dsl::expires_at.eq(expires_at.nullable()))
                .execute(conn)
                .await
                .map_err(Error::from)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NewWorkspaceRetentionJob, RetentionJobOutboxRepository};
    use crate::model::NewWorkspaceFile;
    use crate::query::WorkspaceFileRepository;
    use crate::test_util::TestDatabase;
    use crate::types::{FileKind, Retention};
    use crate::{AsyncConnection, Result};

    #[tokio::test]
    async fn enqueue_claim_then_process_removes_the_row_from_the_pending_set() -> anyhow::Result<()>
    {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        conn.enqueue_retention_job(NewWorkspaceRetentionJob::workspace(seeded.workspace_id))
            .await?;

        conn.transaction(async |conn| -> Result<()> {
            let batch = conn.claim_retention_job_batch(10).await?;
            assert_eq!(batch.len(), 1);
            assert_eq!(batch[0].workspace_id, seeded.workspace_id);
            assert!(batch[0].pipeline_id.is_none());
            conn.mark_retention_job_processed(batch[0].id).await?;
            Ok(())
        })
        .await?;

        let empty = conn
            .transaction(async |conn| conn.claim_retention_job_batch(10).await)
            .await?;
        assert!(empty.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn enqueue_coalesces_a_second_pending_job_for_the_same_scope() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        // Two enqueues for the same (workspace, no pipeline) scope collapse onto
        // one pending row via the partial-unique index.
        conn.enqueue_retention_job(NewWorkspaceRetentionJob::workspace(seeded.workspace_id))
            .await?;
        conn.enqueue_retention_job(NewWorkspaceRetentionJob::workspace(seeded.workspace_id))
            .await?;

        let batch = conn
            .transaction(async |conn| conn.claim_retention_job_batch(10).await)
            .await?;
        assert_eq!(batch.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn reproject_sets_expiry_from_created_at() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_pipeline_and_file().await;
        let mut conn = db.client.get_connection().await?;

        // A fresh original file for the workspace scope.
        let file = conn
            .create_workspace_file(NewWorkspaceFile::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        // Ephemeral reprojection sets `expires_at` to the file's own `created_at`.
        let page = conn
            .reproject_files_expiry_page(
                seeded.workspace_id,
                None,
                FileKind::Original,
                Retention::Ephemeral,
                None,
            )
            .await?;
        assert!(page.contains(&file.id));

        let reread = conn
            .find_workspace_file_by_id(file.id)
            .await?
            .expect("file exists");
        assert_eq!(reread.expires_at, Some(reread.created_at));

        // Persistent reprojection clears the expiry.
        conn.reproject_files_expiry_page(
            seeded.workspace_id,
            None,
            FileKind::Original,
            Retention::Persistent,
            None,
        )
        .await?;
        let reread = conn
            .find_workspace_file_by_id(file.id)
            .await?
            .expect("file exists");
        assert_eq!(reread.expires_at, None);
        Ok(())
    }
}
