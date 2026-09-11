//! Assistant-job outbox drainer.
//!
//! Publishes each pending assistant-reply outbox row onto the assistant
//! work-queue, so a reply queued transactionally with the triggering comment
//! reaches the worker even if the process crashes between the commit and the
//! publish. This is the relay half of the transactional outbox: the comment
//! handler writes the row in the comment's transaction, and this drains it to
//! NATS.

use std::time::Duration;

use nvisy_nats::stream::EventPublisher;
use nvisy_postgres::AsyncConnection;
use nvisy_postgres::model::WorkspaceAssistantJob;
use nvisy_postgres::query::AssistantJobOutboxRepository;
use tokio_util::sync::CancellationToken;

use super::coordinator::AssistantCoordinator;
use super::job::{AssistantJob, AssistantStream};
use crate::response::{Error, Result};
use crate::service::{Infra, Worker};

/// Tracing target for the assistant-job drainer.
const TRACING_TARGET: &str = "nvisy_server::service::assistant::drainer";

/// How often the drainer polls for due jobs. Short, since it is the enqueue
/// latency between addressing the assistant and the worker picking it up.
const TICK_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum jobs drained per tick, bounding the work (and lock hold) per pass.
const DRAIN_BATCH: i64 = 100;

/// Base unit of the retry backoff (seconds): a failed row's next attempt is
/// deferred by `RETRY_BACKOFF_BASE_SECS * attempts` (linear), capped at
/// [`RETRY_BACKOFF_MAX_SECS`].
const RETRY_BACKOFF_BASE_SECS: i64 = 30;

/// Ceiling on the retry backoff (seconds), so a long-failing row still retries
/// periodically rather than backing off unboundedly.
const RETRY_BACKOFF_MAX_SECS: i64 = 60 * 60;

/// How many failed attempts a row gets before the drainer dead-letters it, so a
/// job that can never publish (e.g. an undecodable payload) stops consuming drain
/// cycles instead of retrying forever.
const MAX_ATTEMPTS: i32 = 10;

/// Cap on a single publish, so a slow or unavailable NATS server cannot hold the
/// batch transaction's row locks open indefinitely. A publish that exceeds this is
/// treated as a failed attempt (deferred with a backoff), releasing the locks.
const PUBLISH_TIMEOUT: Duration = Duration::from_secs(5);

/// Drains the assistant-job outbox, publishing each pending job to the work-queue.
pub struct AssistantOutboxDrainer {
    infra: Infra,
    coordinator: AssistantCoordinator,
}

/// The tally of one [`drain_batch`](AssistantOutboxDrainer::drain_batch) pass: of
/// the rows claimed, how many published, how many were deferred for a later retry,
/// and how many were dead-lettered.
struct DrainPass {
    claimed: usize,
    processed: usize,
    deferred: usize,
    dead_lettered: usize,
}

impl Worker for AssistantOutboxDrainer {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "assistant_outbox_drainer"
    }

    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting assistant-job drainer");

        // The timer is the fallback (and cross-instance/crash safety net); the
        // wake signal is the fast path so a job enqueued on this instance drains at
        // once rather than waiting up to TICK_INTERVAL. A wake stored while a pass
        // runs coalesces into one following pass, and the Postgres claim keeps a
        // job single-drained no matter how many instances wake.
        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => self.tick(&cancel).await,
                _ = self.coordinator.notified() => self.tick(&cancel).await,
            }
        }

        tracing::info!(target: TRACING_TARGET, "Assistant-job drainer stopped");
        Ok(())
    }
}

impl AssistantOutboxDrainer {
    /// Creates a new [`AssistantOutboxDrainer`].
    ///
    /// Shares the [`AssistantCoordinator`] with the enqueue-side `AssistantQueue`
    /// so a job committed on this instance wakes this drainer at once.
    pub fn new(infra: Infra, coordinator: AssistantCoordinator) -> Self {
        Self { infra, coordinator }
    }

    /// One drain pass: claim and publish batches until a short page signals the due
    /// set is drained, or until cancellation is requested.
    async fn tick(&self, cancel: &CancellationToken) {
        loop {
            if cancel.is_cancelled() {
                break;
            }
            match self.drain_batch().await {
                Ok(pass) => {
                    if pass.deferred > 0 || pass.dead_lettered > 0 {
                        tracing::warn!(
                            target: TRACING_TARGET,
                            claimed = pass.claimed,
                            processed = pass.processed,
                            deferred = pass.deferred,
                            dead_lettered = pass.dead_lettered,
                            "Assistant-job drain pass had failing jobs",
                        );
                    } else if pass.claimed > 0 {
                        tracing::debug!(target: TRACING_TARGET, processed = pass.processed, "Assistant-job drain pass published jobs");
                    }
                    if pass.claimed < DRAIN_BATCH as usize {
                        break;
                    }
                }
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Assistant-job drain pass failed");
                    break;
                }
            }
        }
    }

    /// Drains one batch: claims due rows and publishes each to the work-queue, all
    /// in one transaction. Returns the [`DrainPass`] tally.
    ///
    /// The transaction holds the claim's `FOR UPDATE SKIP LOCKED` locks through
    /// completion, so the claim and each row's state transition commit atomically
    /// and no other drainer takes the same rows. The publish runs inside the
    /// transaction and gates `mark_processed`: this is at-least-once (a crash after
    /// publish but before commit re-publishes on the next pass), which the worker's
    /// dedup absorbs — the assistant only ever replies once per triggering comment.
    /// A dead-lettered job simply means no reply is posted, so (unlike detection)
    /// there is no domain entity to fail here.
    async fn drain_batch(&self) -> Result<DrainPass> {
        let mut conn = self.infra.postgres.get_connection().await?;

        // Build the stream publisher once per pass rather than per row: it runs the
        // JetStream stream lookup/reconciliation on construction, which need not
        // repeat for each of the (up to `DRAIN_BATCH`) rows.
        let publisher = self.infra.nats.event_publisher::<AssistantStream>().await?;

        conn.transaction(async |conn| {
            let batch = conn.claim_assistant_job_batch(DRAIN_BATCH).await?;
            let mut pass = DrainPass {
                claimed: batch.len(),
                processed: 0,
                deferred: 0,
                dead_lettered: 0,
            };

            for row in batch {
                match publish(&publisher, &row).await {
                    PublishOutcome::Published => {
                        conn.mark_assistant_job_processed(row.id).await?;
                        pass.processed += 1;
                    }
                    // `attempts` counts prior failures; this attempt makes it
                    // `attempts + 1`. Once that reaches the cap, dead-letter the row
                    // instead of deferring it forever. A dead-lettered reply job just
                    // means the assistant never answers this message.
                    PublishOutcome::Failed if row.attempts + 1 >= MAX_ATTEMPTS => {
                        tracing::error!(target: TRACING_TARGET, id = %row.id, comment_id = %row.comment_id, attempts = row.attempts + 1, "Dead-lettering assistant job after too many failed attempts");
                        conn.mark_assistant_job_failed(row.id).await?;
                        pass.dead_lettered += 1;
                    }
                    PublishOutcome::Failed => {
                        conn.defer_assistant_job_attempt(row.id, retry_backoff(row.attempts))
                            .await?;
                        pass.deferred += 1;
                    }
                    // A publish timeout signals NATS is unavailable or hung. Do not
                    // burn `PUBLISH_TIMEOUT` on each remaining row — that could hold
                    // the batch's `FOR UPDATE SKIP LOCKED` locks and the pooled
                    // connection for `DRAIN_BATCH * PUBLISH_TIMEOUT`. Defer this row
                    // and stop the pass; the next tick retries the rest.
                    PublishOutcome::TimedOut => {
                        conn.defer_assistant_job_attempt(row.id, retry_backoff(row.attempts))
                            .await?;
                        pass.deferred += 1;
                        tracing::warn!(target: TRACING_TARGET, id = %row.id, "Assistant-job publish timed out; deferring the rest of the batch");
                        break;
                    }
                }
            }

            Ok::<_, Error>(pass)
        })
        .await
    }
}

/// The result of one publish attempt: published, failed (decode or NATS error),
/// or timed out (NATS unavailable/hung — the caller stops the batch).
enum PublishOutcome {
    /// Published to the work-queue.
    Published,
    /// The payload could not decode or NATS rejected the publish.
    Failed,
    /// The publish exceeded [`PUBLISH_TIMEOUT`]; NATS is likely down.
    TimedOut,
}

/// Decodes a row's job and publishes it to the work-queue with the shared
/// publisher. A decode error or NATS error is [`Failed`](PublishOutcome::Failed);
/// exceeding [`PUBLISH_TIMEOUT`] is [`TimedOut`](PublishOutcome::TimedOut).
async fn publish(
    publisher: &EventPublisher<AssistantStream>,
    row: &WorkspaceAssistantJob,
) -> PublishOutcome {
    let Ok(job) = serde_json::from_value::<AssistantJob>(row.job.clone()) else {
        tracing::error!(target: TRACING_TARGET, id = %row.id, "Failed to decode assistant job");
        return PublishOutcome::Failed;
    };
    // Bound the publish so a hung NATS cannot hold the batch transaction's locks
    // open; a timeout stops the whole pass (see the caller).
    match tokio::time::timeout(PUBLISH_TIMEOUT, publisher.publish(&job)).await {
        Ok(Ok(())) => PublishOutcome::Published,
        Ok(Err(err)) => {
            tracing::warn!(target: TRACING_TARGET, error = %err, id = %row.id, "Failed to publish assistant job; deferring");
            PublishOutcome::Failed
        }
        Err(_elapsed) => PublishOutcome::TimedOut,
    }
}

/// The delay in seconds before a failed row's next attempt: linear in `attempts`
/// (the count before this failure), capped at [`RETRY_BACKOFF_MAX_SECS`].
fn retry_backoff(attempts: i32) -> i64 {
    let steps = i64::from(attempts.max(0)) + 1;
    RETRY_BACKOFF_BASE_SECS
        .saturating_mul(steps)
        .min(RETRY_BACKOFF_MAX_SECS)
}
