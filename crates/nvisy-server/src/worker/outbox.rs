//! The transactional-outbox drainer, shared by every outbox.
//!
//! A request writes an outbox row in the same transaction as the change it
//! announces; an [`OutboxDrainer`] then claims due rows in batches and hands each
//! to its [`OutboxHandler`] — publish a job to its work-queue, or project an
//! event onto its sinks. Each batch runs in one transaction: the claim (`FOR
//! UPDATE SKIP LOCKED`), the handler's database work, and the row's transition
//! commit together, so a competing drainer never takes the same rows and a crash
//! mid-batch rolls back cleanly. A failed row is deferred with a backoff (so it
//! cannot spin at the head of the queue) and dead-lettered once its attempts run
//! out; best-effort side effects run after the batch commits, off the rows that
//! landed.

use std::fmt::Display;
use std::future::Future;
use std::time::Duration;

use nvisy_postgres::query::{OutboxRepository, OutboxRow};
use nvisy_postgres::{AsyncConnection, PgConn};
use serde::de::DeserializeOwned;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::response::{Error, Result};
use crate::service::Infra;
use crate::worker::{Coordinator, Worker, every};

/// Tracing target for the outbox drainers.
const TRACING_TARGET: &str = "nvisy_server::worker::outbox";

/// How often a drainer polls for due rows: the delivery latency when no wake
/// arrives (a row written on another instance, or a missed wake).
const TICK_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum rows drained per batch, bounding the work (and lock hold) per pass.
const DRAIN_BATCH: i64 = 100;

/// Base unit of the retry backoff (seconds): a failed row's next attempt is
/// deferred by `RETRY_BACKOFF_BASE_SECS * attempts` (linear), capped at
/// [`RETRY_BACKOFF_MAX_SECS`].
const RETRY_BACKOFF_BASE_SECS: i64 = 30;

/// Ceiling on the retry backoff (seconds), so a long-failing row still retries
/// periodically rather than backing off unboundedly.
const RETRY_BACKOFF_MAX_SECS: i64 = 60 * 60;

/// How many failed attempts a row gets before it is dead-lettered, so a row that
/// can never be handled (e.g. an undecodable payload) stops consuming drain
/// cycles instead of retrying forever.
const MAX_ATTEMPTS: i32 = 10;

/// Cap on a single [`relay`] publish, so a slow or unavailable NATS server
/// cannot hold the batch transaction's row locks open indefinitely.
const PUBLISH_TIMEOUT: Duration = Duration::from_secs(5);

/// What handling one outbox row came to.
pub enum Handled<D> {
    /// Handled; the row is marked processed. Carries what the post-commit side
    /// effects need, if anything.
    Done(Option<D>),
    /// Failed; the row is deferred, or dead-lettered once its attempts run out.
    Failed,
    /// Timed out talking to NATS, which is likely down: handled like `Failed`,
    /// and the pass stops so the remaining rows are not each held for the
    /// timeout.
    TimedOut,
}

/// What one outbox's rows mean: how to handle a row, and what to do once rows
/// are dead-lettered or committed.
pub trait OutboxHandler: Send + Sync + 'static {
    /// The drainer's worker name.
    const NAME: &'static str;

    /// The outbox table's row.
    type Row: OutboxRow;

    /// What a handled or dead-lettered row hands to [`after_commit`].
    ///
    /// [`after_commit`]: OutboxHandler::after_commit
    type Done: Send;

    /// Handles one row inside the batch transaction.
    fn handle(
        &self,
        conn: &mut PgConn,
        row: &Self::Row,
    ) -> impl Future<Output = Handled<Self::Done>> + Send;

    /// Runs inside the batch transaction when `row` is dead-lettered.
    fn dead_letter(
        &self,
        _conn: &mut PgConn,
        _row: &Self::Row,
    ) -> impl Future<Output = Result<Option<Self::Done>>> + Send {
        async { Ok(None) }
    }

    /// Runs after the batch commits, with what its rows handed over.
    fn after_commit(&self, _done: Vec<Self::Done>) -> impl Future<Output = ()> + Send {
        async {}
    }
}

/// Drains one outbox, handing each due row to its [`OutboxHandler`].
pub struct OutboxDrainer<H> {
    infra: Infra,
    wake: Option<Coordinator>,
    handler: H,
}

/// The tally of one batch: of the rows claimed, how many were handled, deferred
/// for a later retry, and dead-lettered, and whether a NATS timeout stopped it.
struct DrainPass {
    claimed: usize,
    processed: usize,
    deferred: usize,
    dead_lettered: usize,
    timed_out: bool,
}

impl<H: OutboxHandler> OutboxDrainer<H> {
    /// A drainer for `handler`'s outbox. A `wake` shared with the enqueue side
    /// drains a row written on this instance at once instead of on the next
    /// tick.
    pub fn new(infra: Infra, wake: Option<Coordinator>, handler: H) -> Self {
        Self {
            infra,
            wake,
            handler,
        }
    }

    /// One drain pass: claim and handle batches until a short page signals the
    /// due set is drained, a NATS timeout stops it, or cancellation is requested.
    async fn tick(&self, cancel: &CancellationToken) {
        while !cancel.is_cancelled() {
            match self.drain_batch().await {
                Ok(pass) => {
                    if pass.deferred > 0 || pass.dead_lettered > 0 {
                        tracing::warn!(
                            target: TRACING_TARGET,
                            outbox = H::NAME,
                            claimed = pass.claimed,
                            processed = pass.processed,
                            deferred = pass.deferred,
                            dead_lettered = pass.dead_lettered,
                            timed_out = pass.timed_out,
                            "Outbox drain pass had failing rows",
                        );
                    } else if pass.claimed > 0 {
                        tracing::debug!(target: TRACING_TARGET, outbox = H::NAME, processed = pass.processed, "Outbox drain pass handled rows");
                    }
                    if pass.timed_out
                        || i64::try_from(pass.claimed).unwrap_or(i64::MAX) < DRAIN_BATCH
                    {
                        break;
                    }
                }
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, outbox = H::NAME, error = %err, "Outbox drain pass failed");
                    break;
                }
            }
        }
    }

    /// Drains one batch in one transaction, then runs the handler's post-commit
    /// side effects for the rows that landed.
    async fn drain_batch(&self) -> Result<DrainPass> {
        let mut conn = self.infra.postgres.get_connection().await?;

        let (pass, done) = conn
            .transaction(async |conn| {
                let batch = conn.claim_outbox_batch::<H::Row>(DRAIN_BATCH).await?;
                let mut pass = DrainPass {
                    claimed: batch.len(),
                    processed: 0,
                    deferred: 0,
                    dead_lettered: 0,
                    timed_out: false,
                };
                let mut done = Vec::new();

                for row in batch {
                    let handled = self.handler.handle(conn, &row).await;
                    let timed_out = matches!(handled, Handled::TimedOut);
                    match handled {
                        Handled::Done(item) => {
                            conn.mark_outbox_processed(&row).await?;
                            done.extend(item);
                            pass.processed += 1;
                        }
                        // `attempts` counts prior failures; this attempt makes it
                        // `attempts + 1`. Once that reaches the cap, dead-letter
                        // the row instead of deferring it forever.
                        Handled::Failed | Handled::TimedOut
                            if row.attempts() + 1 >= MAX_ATTEMPTS =>
                        {
                            tracing::error!(target: TRACING_TARGET, outbox = H::NAME, id = %row.id(), attempts = row.attempts() + 1, "Dead-lettering outbox row after too many failed attempts");
                            conn.mark_outbox_failed(&row).await?;
                            done.extend(self.handler.dead_letter(conn, &row).await?);
                            pass.dead_lettered += 1;
                        }
                        Handled::Failed | Handled::TimedOut => {
                            conn.defer_outbox_attempt(&row, retry_backoff(row.attempts()))
                                .await?;
                            pass.deferred += 1;
                        }
                    }
                    // Do not burn the timeout on each remaining row — that could
                    // hold the batch's locks and the pooled connection for
                    // `DRAIN_BATCH * PUBLISH_TIMEOUT`. The next tick retries the
                    // rest.
                    if timed_out {
                        pass.timed_out = true;
                        break;
                    }
                }

                Ok::<_, Error>((pass, done))
            })
            .await?;
        // The hook may take its own pooled connection (webhook and notification
        // emitters), so return this one first.
        drop(conn);

        self.handler.after_commit(done).await;
        Ok(pass)
    }
}

impl<H: OutboxHandler> Worker for OutboxDrainer<H> {
    type Error = Error<'static>;

    const NAME: &'static str = H::NAME;

    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        every(TICK_INTERVAL, &cancel, self.wake.as_ref(), async || {
            self.tick(&cancel).await;
        })
        .await;
        Ok(())
    }
}

/// Decodes a job row's payload and publishes it with `publish`, bounded by
/// [`PUBLISH_TIMEOUT`]: the handling shared by the outboxes that relay jobs onto
/// a work-queue.
pub async fn relay<T, D, F, Fut, E>(id: Uuid, payload: &serde_json::Value, publish: F) -> Handled<D>
where
    T: DeserializeOwned,
    F: FnOnce(T) -> Fut,
    Fut: Future<Output = std::result::Result<(), E>>,
    E: Display,
{
    let job = match T::deserialize(payload) {
        Ok(job) => job,
        Err(err) => {
            tracing::error!(target: TRACING_TARGET, error = %err, %id, "Failed to decode outbox job");
            return Handled::Failed;
        }
    };
    match tokio::time::timeout(PUBLISH_TIMEOUT, publish(job)).await {
        Ok(Ok(())) => Handled::Done(None),
        Ok(Err(err)) => {
            tracing::warn!(target: TRACING_TARGET, error = %err, %id, "Failed to publish outbox job");
            Handled::Failed
        }
        Err(_elapsed) => {
            tracing::warn!(target: TRACING_TARGET, %id, "Outbox job publish timed out");
            Handled::TimedOut
        }
    }
}

/// The delay in seconds before a failed row's next attempt: linear in `attempts`
/// (the count before this failure), capped at [`RETRY_BACKOFF_MAX_SECS`], so a
/// transient failure retries soon while a persistently failing row backs off the
/// queue head.
fn retry_backoff(attempts: i32) -> i64 {
    let steps = i64::from(attempts.max(0)) + 1;
    RETRY_BACKOFF_BASE_SECS
        .saturating_mul(steps)
        .min(RETRY_BACKOFF_MAX_SECS)
}
