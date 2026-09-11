//! Event-outbox repository: the write side (insert in the action's transaction)
//! and the drainer side (claim a due batch, mark processed, defer or dead-letter
//! a failure).
//!
//! The drainer runs the claim and the subsequent `mark_*`/`defer_*` inside one
//! transaction per batch (see the outbox drainer), so the `FOR UPDATE SKIP LOCKED`
//! locks are held from claim through completion: no other drainer can take the
//! same rows, and a row's state transition commits atomically with its projection.

use std::future::Future;

use diesel::prelude::*;
use diesel::sql_types::{BigInt, Timestamptz};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{EventOutbox, NewEventOutbox};
use crate::types::OutboxStatus;
use crate::{Error, PgConnection, Result, schema};

/// Read and write operations on the event outbox.
pub trait EventOutboxRepository {
    /// Inserts one outbox row. Called in the same transaction as the action it
    /// records, so the two commit atomically.
    fn insert_event_outbox(
        &mut self,
        row: NewEventOutbox,
    ) -> impl Future<Output = Result<EventOutbox>> + Send;

    /// Claims up to `limit` due pending rows for processing, oldest first.
    ///
    /// Due means unprocessed, not dead-lettered, and past its `next_attempt_at`, so
    /// a row deferred by a backoff is skipped until its time arrives. Locks the
    /// claimed rows with `FOR UPDATE SKIP LOCKED` so concurrent drainers take
    /// disjoint batches without blocking each other; the lock is held for the
    /// caller's transaction. Must run inside that transaction.
    fn claim_outbox_batch(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<EventOutbox>>> + Send;

    /// Marks a row processed (its event durably projected), taking it out of the
    /// pending set. Runs in the drainer's batch transaction.
    fn mark_outbox_processed(&mut self, id: Uuid) -> impl Future<Output = Result<()>> + Send;

    /// Records a failed attempt: increments `attempts` and defers the next attempt
    /// to `now() + backoff_secs` (computed by the database clock, so a drainer's
    /// wall-clock skew cannot mis-schedule it), leaving the row pending for a later
    /// retry. Runs in the drainer's batch transaction.
    fn defer_outbox_attempt(
        &mut self,
        id: Uuid,
        backoff_secs: i64,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Dead-letters a row: increments `attempts` and stamps `failed_at`, taking it
    /// out of the pending set so a poison event (one that can never decode or
    /// project) stops consuming drain cycles. The row is retained for inspection.
    /// Runs in the drainer's batch transaction.
    fn mark_outbox_failed(&mut self, id: Uuid) -> impl Future<Output = Result<()>> + Send;
}

impl EventOutboxRepository for PgConnection {
    async fn insert_event_outbox(&mut self, row: NewEventOutbox) -> Result<EventOutbox> {
        use schema::event_outbox;

        diesel::insert_into(event_outbox::table)
            .values(&row)
            .returning(EventOutbox::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn claim_outbox_batch(&mut self, limit: i64) -> Result<Vec<EventOutbox>> {
        use schema::event_outbox::{self, dsl};

        event_outbox::table
            .filter(dsl::status.eq(OutboxStatus::Pending))
            .filter(dsl::next_attempt_at.le(diesel::dsl::now))
            .order((dsl::next_attempt_at.asc(), dsl::created_at.asc()))
            .limit(limit)
            .select(EventOutbox::as_select())
            .for_update()
            .skip_locked()
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn mark_outbox_processed(&mut self, id: Uuid) -> Result<()> {
        use schema::event_outbox::{self, dsl};

        diesel::update(event_outbox::table.filter(dsl::id.eq(id)))
            .set((
                dsl::status.eq(OutboxStatus::Processed),
                dsl::attempts.eq(dsl::attempts + 1),
            ))
            .execute(self)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    async fn defer_outbox_attempt(&mut self, id: Uuid, backoff_secs: i64) -> Result<()> {
        use schema::event_outbox::{self, dsl};

        // The row stays `Pending`; only its attempt count and next-due time move.
        // `now() + (backoff_secs * interval '1 second')` schedules the next attempt
        // by the database clock, not the drainer's.
        let next_attempt_at = diesel::dsl::sql::<Timestamptz>("now() + (")
            .bind::<BigInt, _>(backoff_secs)
            .sql(" * interval '1 second')");
        diesel::update(event_outbox::table.filter(dsl::id.eq(id)))
            .set((
                dsl::attempts.eq(dsl::attempts + 1),
                dsl::next_attempt_at.eq(next_attempt_at),
            ))
            .execute(self)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    async fn mark_outbox_failed(&mut self, id: Uuid) -> Result<()> {
        use schema::event_outbox::{self, dsl};

        diesel::update(event_outbox::table.filter(dsl::id.eq(id)))
            .set((
                dsl::status.eq(OutboxStatus::Failed),
                dsl::attempts.eq(dsl::attempts + 1),
            ))
            .execute(self)
            .await
            .map_err(Error::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, SelectableHelper};
    use diesel_async::RunQueryDsl;
    use uuid::Uuid;

    use super::{EventOutboxRepository, OutboxStatus, schema};
    use crate::model::{EventOutbox, NewEventOutbox};
    use crate::test_util::TestDatabase;
    use crate::{AsyncConnection, PgConn, Result};

    /// Re-reads an outbox row by id, bypassing the repository (which has no
    /// single-row getter) so tests can assert on its post-transition state.
    async fn reread(conn: &mut PgConn, id: Uuid) -> anyhow::Result<Option<EventOutbox>> {
        use schema::event_outbox::dsl;

        let row = dsl::event_outbox
            .filter(dsl::id.eq(id))
            .select(EventOutbox::as_select())
            .first(conn)
            .await
            .optional()?;
        Ok(row)
    }

    #[tokio::test]
    async fn claim_then_process_removes_the_row_from_the_pending_set() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let row = conn
            .insert_event_outbox(NewEventOutbox::test(seeded.workspace_id, seeded.account_id))
            .await?;

        // The drainer claims and processes in one transaction.
        let processed = conn
            .transaction(async |conn| -> Result<Uuid> {
                let batch = conn.claim_outbox_batch(10).await?;
                assert_eq!(batch.len(), 1);
                assert_eq!(batch[0].id, row.id);
                conn.mark_outbox_processed(batch[0].id).await?;
                Ok(batch[0].id)
            })
            .await?;
        assert_eq!(processed, row.id);

        // Nothing is due any more.
        let empty = conn
            .transaction(async |conn| conn.claim_outbox_batch(10).await)
            .await?;
        assert!(empty.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn defer_pushes_the_row_out_of_the_due_window() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;

        let row = conn
            .insert_event_outbox(NewEventOutbox::test(seeded.workspace_id, seeded.account_id))
            .await?;

        // Claim, then defer the attempt an hour into the future.
        conn.transaction(async |conn| -> Result<()> {
            let batch = conn.claim_outbox_batch(10).await?;
            assert_eq!(batch.len(), 1);
            conn.defer_outbox_attempt(batch[0].id, 3600).await?;
            Ok(())
        })
        .await?;

        // It is still pending but no longer due, so a later claim skips it.
        let due = conn
            .transaction(async |conn| conn.claim_outbox_batch(10).await)
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
            .insert_event_outbox(NewEventOutbox::test(seeded.workspace_id, seeded.account_id))
            .await?;

        conn.transaction(async |conn| -> Result<()> {
            let batch = conn.claim_outbox_batch(10).await?;
            conn.mark_outbox_failed(batch[0].id).await?;
            Ok(())
        })
        .await?;

        // A dead-lettered row is out of the pending set for good.
        let due = conn
            .transaction(async |conn| conn.claim_outbox_batch(10).await)
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
