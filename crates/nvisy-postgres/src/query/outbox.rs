//! Transactional outbox repository, shared by every outbox table: workspace
//! events, detection jobs, and assistant jobs.
//!
//! A row is inserted in the same transaction as the change it announces, so the
//! two commit atomically. A drainer then claims a due batch, publishes each row,
//! and marks it processed, defers it after a failed attempt, or dead-letters it
//! once attempts run out. The drainer runs the claim and every transition inside
//! one transaction per batch, so the `FOR UPDATE SKIP LOCKED` locks are held from
//! claim through completion: no other drainer takes the same rows, and a row's
//! transition commits atomically with its publication.
//!
//! Every outbox table has the same lifecycle columns (`status`, `attempts`,
//! `next_attempt_at`), so one [`OutboxRepository`] serves them all, typed by the
//! row: [`OutboxRow`] and [`NewOutboxRow`] are implemented per table.

use std::future::Future;

use diesel::prelude::*;
use diesel::sql_types::{BigInt, Timestamptz};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceAssistantJob, NewWorkspaceDetectionJob, NewWorkspaceEventOutbox,
    WorkspaceAssistantJob, WorkspaceDetectionJob, WorkspaceEventOutbox,
};
use crate::types::OutboxStatus;
use crate::{PgConnection, Result, schema};

/// A row of an outbox table.
pub trait OutboxRow: Sized + Send + Sync {
    /// The row's id.
    fn id(&self) -> Uuid;

    /// Failed attempts so far.
    fn attempts(&self) -> i32;

    /// Claims up to `limit` due pending rows, oldest first, locking them
    /// `FOR UPDATE SKIP LOCKED`.
    fn claim(conn: &mut PgConnection, limit: i64)
    -> impl Future<Output = Result<Vec<Self>>> + Send;

    /// Moves a row to `status` and counts the attempt.
    fn settle(
        conn: &mut PgConnection,
        id: Uuid,
        status: OutboxStatus,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Counts a failed attempt and pushes the next one `backoff_secs` out, by
    /// the database clock.
    fn defer(
        conn: &mut PgConnection,
        id: Uuid,
        backoff_secs: i64,
    ) -> impl Future<Output = Result<()>> + Send;
}

/// An insertable outbox row.
pub trait NewOutboxRow: Send {
    /// The row this inserts.
    type Row: OutboxRow;

    /// Inserts the row.
    fn insert(self, conn: &mut PgConnection) -> impl Future<Output = Result<Self::Row>> + Send;
}

/// Read and write operations on an outbox table, typed by its row.
pub trait OutboxRepository {
    /// Inserts one outbox row. Called in the same transaction as the change it
    /// announces, so the two commit atomically.
    fn insert_outbox<N: NewOutboxRow>(
        &mut self,
        row: N,
    ) -> impl Future<Output = Result<N::Row>> + Send;

    /// Claims up to `limit` due pending rows for publication, oldest first.
    ///
    /// Due means pending and past its `next_attempt_at`, so a row deferred by a
    /// backoff is skipped until its time arrives. Locks the claimed rows with
    /// `FOR UPDATE SKIP LOCKED` so concurrent drainers take disjoint batches
    /// without blocking each other; the lock is held for the caller's
    /// transaction, so this must run inside it.
    fn claim_outbox_batch<R: OutboxRow>(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<R>>> + Send;

    /// Marks a row processed (its payload durably published), taking it out of
    /// the pending set.
    fn mark_outbox_processed<R: OutboxRow>(
        &mut self,
        row: &R,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Records a failed attempt and defers the next one to `now() +
    /// backoff_secs`, computed by the database clock so a drainer's wall-clock
    /// skew cannot mis-schedule it. The row stays pending.
    fn defer_outbox_attempt<R: OutboxRow>(
        &mut self,
        row: &R,
        backoff_secs: i64,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Dead-letters a row: records the attempt and marks it `Failed`, so a row
    /// that can never publish stops consuming drain cycles. The row is kept for
    /// inspection.
    fn mark_outbox_failed<R: OutboxRow>(
        &mut self,
        row: &R,
    ) -> impl Future<Output = Result<()>> + Send;
}

impl OutboxRepository for PgConnection {
    async fn insert_outbox<N: NewOutboxRow>(&mut self, row: N) -> Result<N::Row> {
        row.insert(self).await
    }

    async fn claim_outbox_batch<R: OutboxRow>(&mut self, limit: i64) -> Result<Vec<R>> {
        R::claim(self, limit).await
    }

    async fn mark_outbox_processed<R: OutboxRow>(&mut self, row: &R) -> Result<()> {
        R::settle(self, row.id(), OutboxStatus::Processed).await
    }

    async fn defer_outbox_attempt<R: OutboxRow>(
        &mut self,
        row: &R,
        backoff_secs: i64,
    ) -> Result<()> {
        R::defer(self, row.id(), backoff_secs).await
    }

    async fn mark_outbox_failed<R: OutboxRow>(&mut self, row: &R) -> Result<()> {
        R::settle(self, row.id(), OutboxStatus::Failed).await
    }
}

/// Implements [`OutboxRow`] and [`NewOutboxRow`] for one outbox table.
macro_rules! outbox_table {
    ($table:ident, $row:ty, $new:ty) => {
        impl OutboxRow for $row {
            fn id(&self) -> Uuid {
                self.id
            }

            fn attempts(&self) -> i32 {
                self.attempts
            }

            async fn claim(conn: &mut PgConnection, limit: i64) -> Result<Vec<Self>> {
                use schema::$table::dsl;

                Ok(dsl::$table
                    .filter(dsl::status.eq(OutboxStatus::Pending))
                    .filter(dsl::next_attempt_at.le(diesel::dsl::now))
                    .order((dsl::next_attempt_at.asc(), dsl::created_at.asc()))
                    .limit(limit)
                    .select(<$row>::as_select())
                    .for_update()
                    .skip_locked()
                    .load(conn)
                    .await?)
            }

            async fn settle(conn: &mut PgConnection, id: Uuid, status: OutboxStatus) -> Result<()> {
                use schema::$table::dsl;

                diesel::update(dsl::$table.filter(dsl::id.eq(id)))
                    .set((dsl::status.eq(status), dsl::attempts.eq(dsl::attempts + 1)))
                    .execute(conn)
                    .await?;
                Ok(())
            }

            async fn defer(conn: &mut PgConnection, id: Uuid, backoff_secs: i64) -> Result<()> {
                use schema::$table::dsl;

                // The row stays `Pending`; only its attempt count and next-due
                // time move.
                let next_attempt_at = diesel::dsl::sql::<Timestamptz>("now() + (")
                    .bind::<BigInt, _>(backoff_secs)
                    .sql(" * interval '1 second')");
                diesel::update(dsl::$table.filter(dsl::id.eq(id)))
                    .set((
                        dsl::attempts.eq(dsl::attempts + 1),
                        dsl::next_attempt_at.eq(next_attempt_at),
                    ))
                    .execute(conn)
                    .await?;
                Ok(())
            }
        }

        impl NewOutboxRow for $new {
            type Row = $row;

            async fn insert(self, conn: &mut PgConnection) -> Result<$row> {
                Ok(diesel::insert_into(schema::$table::table)
                    .values(&self)
                    .returning(<$row>::as_returning())
                    .get_result(conn)
                    .await?)
            }
        }
    };
}

outbox_table!(
    workspace_event_outbox,
    WorkspaceEventOutbox,
    NewWorkspaceEventOutbox
);
outbox_table!(
    workspace_detection_jobs,
    WorkspaceDetectionJob,
    NewWorkspaceDetectionJob
);
outbox_table!(
    workspace_assistant_jobs,
    WorkspaceAssistantJob,
    NewWorkspaceAssistantJob
);

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, SelectableHelper};
    use diesel_async::RunQueryDsl;
    use uuid::Uuid;

    use super::{NewOutboxRow, OutboxRepository, OutboxRow, OutboxStatus, schema};
    use crate::model::{
        NewWorkspaceAssistantJob, NewWorkspaceDetection, NewWorkspaceDetectionJob,
        NewWorkspaceEventOutbox, NewWorkspaceReview, NewWorkspaceReviewComment,
        WorkspaceEventOutbox,
    };
    use crate::query::{
        WorkspaceDetectionRepository, WorkspaceReviewCommentRepository, WorkspaceReviewRepository,
    };
    use crate::test_util::TestDatabase;
    use crate::{AsyncConnection, PgConn, Result};

    /// Re-reads an event outbox row by id, bypassing the repository (which has
    /// no single-row getter) so tests can assert on its post-transition state.
    async fn reread(conn: &mut PgConn, id: Uuid) -> anyhow::Result<Option<WorkspaceEventOutbox>> {
        use schema::workspace_event_outbox::dsl;

        Ok(dsl::workspace_event_outbox
            .filter(dsl::id.eq(id))
            .select(WorkspaceEventOutbox::as_select())
            .first(conn)
            .await
            .optional()?)
    }

    /// Inserts a row, then claims and processes it in one transaction, and
    /// checks nothing is due afterwards.
    async fn claim_and_process<N: NewOutboxRow>(conn: &mut PgConn, new: N) -> anyhow::Result<()> {
        let row = conn.insert_outbox(new).await?;
        let id = row.id();

        conn.transaction(async |conn| -> Result<()> {
            let batch = conn.claim_outbox_batch::<N::Row>(10).await?;
            assert_eq!(batch.len(), 1);
            assert_eq!(batch[0].id(), id);
            conn.mark_outbox_processed(&batch[0]).await
        })
        .await?;

        let empty = conn
            .transaction(async |conn| conn.claim_outbox_batch::<N::Row>(10).await)
            .await?;
        assert!(empty.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn event_outbox_claim_then_process() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        claim_and_process(
            &mut conn,
            NewWorkspaceEventOutbox::test(seeded.workspace_id, seeded.account_id),
        )
        .await
    }

    #[tokio::test]
    async fn detection_jobs_claim_then_process() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;
        let detection = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;

        claim_and_process(&mut conn, NewWorkspaceDetectionJob::test(detection.id)).await
    }

    #[tokio::test]
    async fn assistant_jobs_claim_then_process() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;
        let review = conn
            .create_review(NewWorkspaceReview::test(
                seeded.workspace_id,
                seeded.document_id,
                seeded.account_id,
            ))
            .await?;
        let comment = conn
            .create_comment(NewWorkspaceReviewComment {
                parent_id: None,
                review_id: review.id,
                author_account_id: seeded.account_id,
                body: "@assistant help".to_owned(),
            })
            .await?;

        claim_and_process(&mut conn, NewWorkspaceAssistantJob::test(comment.id)).await
    }

    #[tokio::test]
    async fn defer_pushes_the_row_out_of_the_due_window() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let row = conn
            .insert_outbox(NewWorkspaceEventOutbox::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        conn.transaction(async |conn| -> Result<()> {
            let batch = conn.claim_outbox_batch::<WorkspaceEventOutbox>(10).await?;
            assert_eq!(batch.len(), 1);
            conn.defer_outbox_attempt(&batch[0], 3600).await
        })
        .await?;

        let due = conn
            .transaction(async |conn| conn.claim_outbox_batch::<WorkspaceEventOutbox>(10).await)
            .await?;
        assert!(due.is_empty(), "deferred row must not be due yet");

        let reread = reread(&mut conn, row.id).await?.expect("row exists");
        assert_eq!(reread.status, OutboxStatus::Pending);
        assert_eq!(reread.attempts, 1);
        Ok(())
    }

    #[tokio::test]
    async fn mark_failed_dead_letters_the_row() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let row = conn
            .insert_outbox(NewWorkspaceEventOutbox::test(
                seeded.workspace_id,
                seeded.account_id,
            ))
            .await?;

        conn.transaction(async |conn| -> Result<()> {
            let batch = conn.claim_outbox_batch::<WorkspaceEventOutbox>(10).await?;
            conn.mark_outbox_failed(&batch[0]).await
        })
        .await?;

        let due = conn
            .transaction(async |conn| conn.claim_outbox_batch::<WorkspaceEventOutbox>(10).await)
            .await?;
        assert!(due.is_empty());

        let reread = reread(&mut conn, row.id)
            .await?
            .expect("row retained for inspection");
        assert_eq!(reread.status, OutboxStatus::Failed);
        assert_eq!(reread.attempts, 1);
        Ok(())
    }
}
