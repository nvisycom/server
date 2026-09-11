//! Retention-backfill outbox drainer.
//!
//! Reprojects the `expires_at` of files already stored under a scope whose
//! retention policy changed. The settings/override handler commits a scope-only
//! job row in its own transaction; this drainer claims those jobs and rewrites
//! each affected file's `expires_at` from the file's own `created_at` under the
//! *current* policy, in bounded keyset-paged batches — so the reprojection never
//! holds locks on an unbounded row set inside a request.

use std::time::Duration;

use nvisy_postgres::AsyncConnection;
use nvisy_postgres::model::WorkspaceRetentionJob;
use nvisy_postgres::query::{
    RetentionJobOutboxRepository, WorkspacePipelineRepository, WorkspaceRepository,
};
use nvisy_postgres::types::{FileKind, Retention, RetentionScope, RetentionSettings};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::coordinator::RetentionBackfillCoordinator;
use crate::response::{Error, Result};
use crate::service::{Infra, Worker};

/// Tracing target for the retention-backfill drainer.
const TRACING_TARGET: &str = "nvisy_server::service::retention::drainer";

/// How often the drainer polls for due jobs.
const TICK_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum jobs drained per tick, bounding the work (and lock hold) per pass.
const DRAIN_BATCH: i64 = 50;

/// Base unit of the retry backoff (seconds): a failed row's next attempt is
/// deferred by `RETRY_BACKOFF_BASE_SECS * attempts` (linear), capped at
/// [`RETRY_BACKOFF_MAX_SECS`].
const RETRY_BACKOFF_BASE_SECS: i64 = 30;

/// Ceiling on the retry backoff (seconds), so a long-failing row still retries
/// periodically rather than backing off unboundedly.
const RETRY_BACKOFF_MAX_SECS: i64 = 60 * 60;

/// How many failed attempts a row gets before the drainer dead-letters it, so a
/// job that can never apply stops consuming drain cycles instead of retrying
/// forever.
const MAX_ATTEMPTS: i32 = 10;

/// The scopes reprojected for a workspace-wide job: every retention scope paired
/// with the file kind it governs (review audits share the audit-logs scope).
const WORKSPACE_SCOPES: [(RetentionScope, FileKind); 5] = [
    (RetentionScope::OriginalDocuments, FileKind::Original),
    (RetentionScope::RedactedDocuments, FileKind::Redacted),
    (RetentionScope::AuditLogs, FileKind::Audit),
    (RetentionScope::AuditLogs, FileKind::Review),
    (RetentionScope::Intermediates, FileKind::Intermediate),
];

/// The scopes reprojected for a pipeline job: only the kinds a pipeline produces
/// (originals are ingested, never produced, so a pipeline override never governs
/// them).
const PIPELINE_SCOPES: [(RetentionScope, FileKind); 4] = [
    (RetentionScope::RedactedDocuments, FileKind::Redacted),
    (RetentionScope::AuditLogs, FileKind::Audit),
    (RetentionScope::AuditLogs, FileKind::Review),
    (RetentionScope::Intermediates, FileKind::Intermediate),
];

/// Drains the retention-backfill outbox, reprojecting each scope's files.
pub struct RetentionBackfillDrainer {
    infra: Infra,
    coordinator: RetentionBackfillCoordinator,
}

impl Worker for RetentionBackfillDrainer {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "retention_backfill_drainer"
    }

    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting retention-backfill drainer");

        // The timer is the fallback (and cross-instance/crash safety net); the
        // wake signal is the fast path so a job committed on this instance drains
        // at once. A wake stored while a pass runs coalesces into one following
        // pass, and the Postgres claim keeps a job single-drained across the fleet.
        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => self.tick(&cancel).await,
                _ = self.coordinator.notified() => self.tick(&cancel).await,
            }
        }

        tracing::info!(target: TRACING_TARGET, "Retention-backfill drainer stopped");
        Ok(())
    }
}

impl RetentionBackfillDrainer {
    /// Creates a new [`RetentionBackfillDrainer`].
    ///
    /// Shares the [`RetentionBackfillCoordinator`] with the enqueue-side handlers
    /// so a job committed on this instance wakes this drainer at once.
    pub fn new(infra: Infra, coordinator: RetentionBackfillCoordinator) -> Self {
        Self { infra, coordinator }
    }

    /// One drain pass: claim and apply batches until a short page signals the due
    /// set is drained, or until cancellation is requested.
    async fn tick(&self, cancel: &CancellationToken) {
        loop {
            if cancel.is_cancelled() {
                break;
            }
            match self.drain_batch().await {
                Ok(claimed) => {
                    if claimed < DRAIN_BATCH as usize {
                        break;
                    }
                }
                Err(err) => {
                    tracing::error!(target: TRACING_TARGET, error = %err, "Retention-backfill drain pass failed");
                    break;
                }
            }
        }
    }

    /// Drains one batch: claims due rows and applies each, all in one transaction.
    /// Returns the number of rows claimed.
    ///
    /// The transaction holds the claim's `FOR UPDATE SKIP LOCKED` locks through
    /// completion, so no other drainer takes the same rows and each row's state
    /// transition commits with its reprojection. A row is dead-lettered after
    /// [`MAX_ATTEMPTS`]; a dead-lettered job just means some files keep a stale
    /// `expires_at` until the next policy change re-enqueues the scope.
    async fn drain_batch(&self) -> Result<usize> {
        let mut conn = self.infra.postgres.get_connection().await?;

        conn.transaction(async |conn| {
            let batch = conn.claim_retention_job_batch(DRAIN_BATCH).await?;
            let claimed = batch.len();

            for job in batch {
                match apply_job(conn, &job).await {
                    Ok(()) => {
                        conn.mark_retention_job_processed(job.id).await?;
                    }
                    Err(err) if job.attempts + 1 >= MAX_ATTEMPTS => {
                        tracing::error!(target: TRACING_TARGET, id = %job.id, workspace_id = %job.workspace_id, error = %err, attempts = job.attempts + 1, "Dead-lettering retention-backfill job after too many failed attempts");
                        conn.mark_retention_job_failed(job.id).await?;
                    }
                    Err(err) => {
                        tracing::warn!(target: TRACING_TARGET, id = %job.id, error = %err, "Retention-backfill job failed; deferring");
                        conn.defer_retention_job_attempt(job.id, retry_backoff(job.attempts))
                            .await?;
                    }
                }
            }

            Ok::<_, Error>(claimed)
        })
        .await
    }
}

/// Reprojects every file the job's scope governs to the current policy. Resolves
/// the workspace baseline (and, for a pipeline scope, its override) once, then
/// reprojects each `(scope, kind)` in bounded, keyset-paged batches.
async fn apply_job(conn: &mut nvisy_postgres::PgConn, job: &WorkspaceRetentionJob) -> Result<()> {
    let baseline: RetentionSettings = conn
        .find_workspace_by_id(job.workspace_id)
        .await?
        .map(|workspace| workspace.settings.or_default().retention)
        // A workspace deleted between enqueue and drain cascades its jobs away, so
        // this is only reachable in a race; the default is a safe fallback.
        .unwrap_or_default();

    // A pipeline scope resolves against that pipeline's current override; a
    // workspace scope resolves against the baseline alone.
    let scopes: &[(RetentionScope, FileKind)] = match job.pipeline_id {
        Some(_) => &PIPELINE_SCOPES,
        None => &WORKSPACE_SCOPES,
    };
    let override_ = match job.pipeline_id {
        Some(pipeline_id) => conn
            .find_pipeline_by_id(pipeline_id)
            .await?
            .and_then(|pipeline| pipeline.metadata.or_default().retention),
        None => None,
    };

    for &(scope, kind) in scopes {
        let retention = baseline.resolve(scope, override_.as_ref());
        reproject_scope(conn, job.workspace_id, job.pipeline_id, kind, retention).await?;
    }
    Ok(())
}

/// Reprojects one `(scope, kind)` to `retention`, paging by file id until the
/// scope is drained so no single `UPDATE` touches an unbounded row set.
async fn reproject_scope(
    conn: &mut nvisy_postgres::PgConn,
    workspace_id: Uuid,
    pipeline_id: Option<Uuid>,
    kind: FileKind,
    retention: Retention,
) -> Result<()> {
    let mut after: Option<Uuid> = None;
    loop {
        let page = conn
            .reproject_files_expiry_page(workspace_id, pipeline_id, kind, retention, after)
            .await?;
        let Some(&last) = page.last() else {
            break;
        };
        after = Some(last);
    }
    Ok(())
}

/// The delay in seconds before a failed row's next attempt: linear in `attempts`
/// (the count before this failure), capped at [`RETRY_BACKOFF_MAX_SECS`].
fn retry_backoff(attempts: i32) -> i64 {
    let steps = i64::from(attempts.max(0)) + 1;
    RETRY_BACKOFF_BASE_SECS
        .saturating_mul(steps)
        .min(RETRY_BACKOFF_MAX_SECS)
}
