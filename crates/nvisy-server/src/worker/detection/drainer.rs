//! Detection-job outbox drainer.
//!
//! Publishes each pending detection-job outbox row onto the detection work-queue,
//! so an analysis queued transactionally with its detection reaches the worker
//! even if the process crashes between the commit and the publish. This is the
//! relay half of the transactional outbox: the create-detection handler writes
//! the row in the detection's transaction, and this drains it to NATS.

use std::time::Duration;

use nvisy_postgres::AsyncConnection;
use nvisy_postgres::model::{UpdateWorkspaceDetection, WorkspaceDetectionJob};
use nvisy_postgres::query::{OutboxRepository, WorkspaceDetectionRepository};
use nvisy_postgres::types::{DetectionMetadata, DetectionStatus, Json};
use tokio_util::sync::CancellationToken;

use super::job::{DetectionJob, broadcast_status, enqueue};
use crate::response::{Error, Result};
use crate::service::Infra;
use crate::worker::{Coordinator, Worker};

/// Tracing target for the detection-job drainer.
const TRACING_TARGET: &str = "nvisy_server::worker::detection";

/// How often the drainer polls for due jobs. Short, since it is the enqueue
/// latency between creating a detection and the worker picking it up.
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

/// The failure reason recorded on a detection whose job the drainer gave up
/// publishing, so the detection's terminal state explains why it never analyzed.
const DEAD_LETTER_REASON: &str = "Detection could not be queued for analysis";

/// Cap on a single publish, so a slow or unavailable NATS server cannot hold the
/// batch transaction's row locks open indefinitely. A publish that exceeds this is
/// treated as a failed attempt (deferred with a backoff), releasing the locks.
const PUBLISH_TIMEOUT: Duration = Duration::from_secs(5);

/// Drains the detection-job outbox, publishing each pending job to the work-queue.
pub struct DetectionOutboxDrainer {
    infra: Infra,
    coordinator: Coordinator,
}

/// The tally of one [`drain_batch`] pass: of the rows claimed, how many
/// published, how many were deferred for a later retry, and how many were
/// dead-lettered.
///
/// [`drain_batch`]: DetectionOutboxDrainer::drain_batch
struct DrainPass {
    claimed: usize,
    processed: usize,
    deferred: usize,
    dead_lettered: usize,
}

impl Worker for DetectionOutboxDrainer {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "detection_outbox_drainer"
    }

    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting detection-job drainer");

        // The timer is the fallback (and cross-instance/crash safety net); the
        // wake signal is the fast path so a job enqueued on this instance drains
        // at once rather than waiting up to TICK_INTERVAL. A wake stored while a
        // pass runs coalesces into one following pass, and the Postgres claim
        // keeps a job single-drained no matter how many instances wake.
        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                _ = ticker.tick() => self.tick(&cancel).await,
                () = self.coordinator.notified() => self.tick(&cancel).await,
            }
        }

        Ok(())
    }
}

impl DetectionOutboxDrainer {
    /// Creates a new [`DetectionOutboxDrainer`].
    ///
    /// Shares the [`Coordinator`] with the enqueue-side [`DetectionQueue`] so a
    /// job committed on this instance wakes this drainer at once.
    ///
    /// [`DetectionQueue`]: crate::service::DetectionQueue
    #[must_use]
    pub fn new(infra: Infra, coordinator: Coordinator) -> Self {
        Self { infra, coordinator }
    }

    /// One drain pass: claim and publish batches until a short page signals the
    /// due set is drained, or until cancellation is requested.
    ///
    /// The cancellation check between batches keeps shutdown prompt even under a
    /// sustained backlog.
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
                            "Detection-job drain pass had failing jobs",
                        );
                    } else if pass.claimed > 0 {
                        tracing::debug!(target: TRACING_TARGET, processed = pass.processed, "Detection-job drain pass published jobs");
                    }
                    if i64::try_from(pass.claimed).unwrap_or(i64::MAX) < DRAIN_BATCH {
                        break;
                    }
                }
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Detection-job drain pass failed");
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
    /// claim-based dedup absorbs — the same detection is only ever analyzed once.
    async fn drain_batch(&self) -> Result<DrainPass> {
        let mut conn = self.infra.postgres.get_connection().await?;

        let outcome = conn
            .transaction(async |conn| {
                let batch = conn.claim_outbox_batch::<WorkspaceDetectionJob>(DRAIN_BATCH).await?;
                let mut pass = DrainPass {
                    claimed: batch.len(),
                    processed: 0,
                    deferred: 0,
                    dead_lettered: 0,
                };
                // Detections failed by a dead-lettered job, so their `Failed`
                // status can be broadcast after the transaction commits.
                let mut dead_lettered = Vec::new();

                for row in batch {
                    match self.publish(&row).await {
                        PublishOutcome::Published => {
                            conn.mark_outbox_processed(&row).await?;
                            pass.processed += 1;
                        }
                        // `attempts` counts prior failures; this attempt makes it
                        // `attempts + 1`. Once that reaches the cap, dead-letter the
                        // row instead of deferring it forever. The job never
                        // published, so the worker will never drive its detection to
                        // a terminal state: fail the detection here too (same
                        // transaction) so it does not hang `Pending` forever. The
                        // guard on `Pending` makes it a no-op in the unlikely case a
                        // worker already claimed the detection.
                        PublishOutcome::Failed if row.attempts + 1 >= MAX_ATTEMPTS => {
                            tracing::error!(target: TRACING_TARGET, id = %row.id, detection_id = %row.detection_id, attempts = row.attempts + 1, "Dead-lettering detection job after too many failed attempts; failing the detection");
                            conn.mark_outbox_failed(&row).await?;
                            let metadata = DetectionMetadata {
                                error: Some(DEAD_LETTER_REASON.to_owned()),
                                ..Default::default()
                            };
                            // Only broadcast `Failed` if this actually transitioned
                            // the detection. A failed publish (e.g. a lost ack) does not
                            // prove the job never reached a worker, so one may own the
                            // detection (no longer `Pending`); the guard then no-ops
                            // and we must not announce a false terminal status.
                            let failed = conn
                                .fail_pending_detection(
                                    row.detection_id,
                                    UpdateWorkspaceDetection {
                                        metadata: Some(Json::encode(&metadata)),
                                        ..Default::default()
                                    },
                                )
                                .await?;
                            if failed {
                                dead_lettered.push(row.detection_id);
                            }
                            pass.dead_lettered += 1;
                        }
                        PublishOutcome::Failed => {
                            conn.defer_outbox_attempt(&row, retry_backoff(row.attempts))
                                .await?;
                            pass.deferred += 1;
                        }
                        // A publish timeout signals NATS is unavailable or hung. Do
                        // not burn `PUBLISH_TIMEOUT` on each remaining row — that
                        // could hold the batch's `FOR UPDATE SKIP LOCKED` locks and
                        // the pooled connection for `DRAIN_BATCH * PUBLISH_TIMEOUT`.
                        // Defer this row and stop the pass; the next tick retries
                        // the rest.
                        PublishOutcome::TimedOut => {
                            conn.defer_outbox_attempt(&row, retry_backoff(row.attempts))
                                .await?;
                            pass.deferred += 1;
                            tracing::warn!(target: TRACING_TARGET, id = %row.id, "Detection-job publish timed out; deferring the rest of the batch");
                            break;
                        }
                    }
                }

                Ok::<_, Error>((pass, dead_lettered))
            })
            .await?;

        // Announce the terminal status of each detection this pass actually failed
        // (only those the guarded transition committed as `Failed` are collected),
        // after its transaction commits. Best-effort: the detection row is
        // authoritative, so a dropped broadcast is recoverable by a re-read.
        let (pass, dead_lettered) = outcome;
        for detection_id in dead_lettered {
            broadcast_status(&self.infra, detection_id, DetectionStatus::Failed).await;
        }

        Ok(pass)
    }

    /// Decodes a row's job and publishes it to the work-queue. A decode error or
    /// NATS error is [`Failed`](PublishOutcome::Failed); exceeding
    /// [`PUBLISH_TIMEOUT`] is [`TimedOut`](PublishOutcome::TimedOut).
    async fn publish(&self, row: &WorkspaceDetectionJob) -> PublishOutcome {
        let job = match serde_json::from_value::<DetectionJob>(row.job.clone()) {
            Ok(job) => job,
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, error = %err, id = %row.id, "Failed to decode detection job");
                return PublishOutcome::Failed;
            }
        };
        // Bound the publish so a hung NATS cannot hold the batch transaction's
        // locks open; a timeout stops the whole pass (see the caller).
        match tokio::time::timeout(PUBLISH_TIMEOUT, enqueue(&self.infra, job)).await {
            Ok(Ok(())) => PublishOutcome::Published,
            Ok(Err(err)) => {
                tracing::warn!(target: TRACING_TARGET, error = %err, id = %row.id, "Failed to publish detection job; deferring");
                PublishOutcome::Failed
            }
            Err(_elapsed) => PublishOutcome::TimedOut,
        }
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

/// The delay in seconds before a failed row's next attempt: linear in `attempts`
/// (the count before this failure), capped at [`RETRY_BACKOFF_MAX_SECS`].
fn retry_backoff(attempts: i32) -> i64 {
    let steps = i64::from(attempts.max(0)) + 1;
    RETRY_BACKOFF_BASE_SECS
        .saturating_mul(steps)
        .min(RETRY_BACKOFF_MAX_SECS)
}
