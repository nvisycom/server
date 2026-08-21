//! Event-outbox repository: the write side (insert in the action's transaction)
//! and the drainer side (claim a batch, mark processed, record a failure).

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
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

    /// Claims up to `limit` pending rows for processing, oldest first.
    ///
    /// Locks the claimed rows with `FOR UPDATE SKIP LOCKED` so concurrent
    /// drainers (across server instances) take disjoint batches without blocking
    /// each other. The lock is held for the caller's transaction, so the drainer
    /// runs the claim and the subsequent `mark_*` in one transaction per batch.
    fn claim_outbox_batch(
        &mut self,
        limit: i64,
    ) -> impl Future<Output = PgResult<Vec<EventOutbox>>> + Send;

    /// Marks a row processed (delivered to all its sinks), taking it out of the
    /// pending set.
    fn mark_outbox_processed(&mut self, id: Uuid) -> impl Future<Output = PgResult<()>> + Send;

    /// Records a failed delivery attempt: increments `attempts` and leaves the
    /// row pending for a later retry.
    fn record_outbox_failure(&mut self, id: Uuid) -> impl Future<Output = PgResult<()>> + Send;
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

    async fn record_outbox_failure(&mut self, id: Uuid) -> PgResult<()> {
        use schema::event_outbox::{self, dsl};

        diesel::update(event_outbox::table.filter(dsl::id.eq(id)))
            .set(dsl::attempts.eq(dsl::attempts + 1))
            .execute(self)
            .await
            .map_err(PgError::from)?;
        Ok(())
    }
}
