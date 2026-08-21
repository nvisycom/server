//! Event-outbox repository: the write side (insert in the action's transaction)
//! and the drainer side (claim a due batch, mark processed, defer a failure).
//!
//! The drainer runs the claim and the subsequent `mark_*` inside one transaction
//! per batch (see the outbox drainer), so the `FOR UPDATE SKIP LOCKED` locks are
//! held from claim through completion: no other drainer can take the same rows,
//! and a row's state transition commits atomically with its projection.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::SignedDuration;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::model::{EventOutbox, NewEventOutbox};
use crate::{PgConnection, PgError, PgResult, schema};

/// Read and write operations on the event outbox.
pub trait EventOutboxRepository {
    /// Inserts one outbox row. Called in the same transaction as the action it
    /// records, so the two commit atomically.
    fn insert_event_outbox(
        &mut self,
        row: NewEventOutbox,
    ) -> impl Future<Output = PgResult<EventOutbox>> + Send;

    /// Claims up to `limit` due pending rows for processing, oldest first.
    ///
    /// Due means unprocessed and past its `next_attempt_at`, so a row deferred by
    /// a backoff is skipped until its time arrives. Locks the claimed rows with
    /// `FOR UPDATE SKIP LOCKED` so concurrent drainers take disjoint batches
    /// without blocking each other; the lock is held for the caller's
    /// transaction. Must run inside that transaction.
    fn claim_outbox_batch(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<EventOutbox>>> + Send;

    /// Marks a row processed (its event durably projected), taking it out of the
    /// pending set. Runs in the drainer's batch transaction.
    fn mark_outbox_processed(&mut self, id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Records a failed attempt: increments `attempts` and defers the next
    /// attempt by `backoff`, leaving the row pending for a later retry. Runs in
    /// the drainer's batch transaction.
    fn defer_outbox_attempt(
        &mut self,
        id: Uuid,
        backoff: SignedDuration,
    ) -> impl Future<Output = PgResult<()>> + Send;
}

impl EventOutboxRepository for PgConnection {
    async fn insert_event_outbox(&mut self, row: NewEventOutbox) -> PgResult<EventOutbox> {
        use schema::event_outbox;

        diesel::insert_into(event_outbox::table)
            .values(&row)
            .returning(EventOutbox::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn claim_outbox_batch(&mut self, limit: i64) -> PgResult<Vec<EventOutbox>> {
        use schema::event_outbox::{self, dsl};

        event_outbox::table
            .filter(dsl::processed_at.is_null())
            .filter(dsl::next_attempt_at.le(diesel::dsl::now))
            .order(dsl::created_at.asc())
            .limit(limit)
            .select(EventOutbox::as_select())
            .for_update()
            .skip_locked()
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn mark_outbox_processed(&mut self, id: Uuid) -> PgResult<()> {
        use schema::event_outbox::{self, dsl};

        diesel::update(event_outbox::table.filter(dsl::id.eq(id)))
            .set((
                dsl::processed_at.eq(diesel::dsl::now),
                dsl::attempts.eq(dsl::attempts + 1),
            ))
            .execute(self)
            .await
            .map_err(PgError::from)?;
        Ok(())
    }

    async fn defer_outbox_attempt(&mut self, id: Uuid, backoff: SignedDuration) -> PgResult<()> {
        use schema::event_outbox::{self, dsl};

        let next_attempt_at: Timestamp = (jiff::Timestamp::now() + backoff).into();
        diesel::update(event_outbox::table.filter(dsl::id.eq(id)))
            .set((
                dsl::attempts.eq(dsl::attempts + 1),
                dsl::next_attempt_at.eq(next_attempt_at),
            ))
            .execute(self)
            .await
            .map_err(PgError::from)?;
        Ok(())
    }
}
