//! File reaper: reclaims stored files and their backing objects.
//!
//! Every stored object — original documents, redacted outputs, and audit blobs —
//! is a `workspace_files` row. The reaper runs two indexed sweeps each tick, both
//! ending in the shared [`RunBlobStore::purge_file`] teardown (soft-delete the
//! row, purge the object, stamp `purged_at`):
//!
//! - **Expire**: live files whose retention window has elapsed (`expires_at <
//!   now()`, from the precomputed per-file rule).
//! - **Reconcile**: already soft-deleted files whose object was never reclaimed
//!   (`deleted_at IS NOT NULL AND purged_at IS NULL`) — a best-effort purge that
//!   failed, or a delete path that could not reach the object store. Retried
//!   until `purged_at` is stamped, so a transient object-store outage self-heals.

use std::time::Duration;

use nvisy_postgres::query::{ExpiredFileRef, WorkspaceFileRepository};
use tokio_util::sync::CancellationToken;

use crate::handler::Result;
use crate::service::{Infra, PurgeOutcome, RunBlobStore, Worker};

/// Tracing target for the file reaper.
const TRACING_TARGET: &str = "nvisy_server::worker::reaper";

/// How often the reaper runs. Retention is day-granular, so an hourly sweep is
/// responsive without being costly.
const TICK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Maximum files purged per sweep pass, bounding the work per tick.
const SWEEP_BATCH: i64 = 500;

/// Periodically reclaims expired files and reconciles orphaned objects.
pub struct FileReaper {
    infra: Infra,
    blob: RunBlobStore,
}

impl Worker for FileReaper {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "file_reaper"
    }

    /// Runs the reaper until cancelled, logging its lifecycle.
    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting file reaper");

        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => self.tick().await,
            }
        }

        tracing::info!(target: TRACING_TARGET, "File reaper stopped");
        Ok(())
    }
}

impl FileReaper {
    /// Creates a new [`FileReaper`].
    pub fn new(infra: Infra) -> Self {
        let blob = RunBlobStore::new(infra.clone());
        Self { infra, blob }
    }

    /// One reaper tick: expire due files, then reconcile any orphaned objects.
    /// Reconcile runs second so it also catches any object a failed expiry in the
    /// same tick left behind.
    async fn tick(&self) {
        if let Err(err) = self.sweep(Sweep::Expire).await {
            tracing::error!(target: TRACING_TARGET, error = %err, "Expiry sweep failed");
        }
        if let Err(err) = self.sweep(Sweep::Reconcile).await {
            tracing::error!(target: TRACING_TARGET, error = %err, "Reconcile sweep failed");
        }
    }

    /// Purges a sweep's files in `SWEEP_BATCH`-sized pages.
    ///
    /// A successful [`purge_file`](RunBlobStore::purge_file) takes the row out of
    /// the sweep's result set (setting `deleted_at`/`purged_at`), so the loop
    /// advances on *successful* purges rather than rows fetched: it stops once a
    /// page is short (nothing more due) or a full page made no progress (every
    /// file in it failed), logging a stuck batch once per tick instead of
    /// spinning on it.
    async fn sweep(&self, sweep: Sweep) -> Result<()> {
        loop {
            let batch = {
                let mut conn = self.infra.postgres.get_connection().await?;
                match sweep {
                    Sweep::Expire => conn.files_due_for_expiry(SWEEP_BATCH).await?,
                    Sweep::Reconcile => conn.files_pending_purge(SWEEP_BATCH).await?,
                }
            };

            let fetched = batch.len() as i64;
            let mut purged = 0i64;
            for file in batch {
                match self.purge_file(&file).await {
                    // Only a confirmed reclamation is progress. A pending purge
                    // leaves the row in the sweep's set, so counting it would
                    // re-fetch the same batch forever.
                    Ok(PurgeOutcome::Purged) => purged += 1,
                    Ok(PurgeOutcome::Pending) => {}
                    Err(err) => tracing::error!(
                        target: TRACING_TARGET,
                        sweep = sweep.label(),
                        file_id = %file.id,
                        error = %err,
                        "Failed to purge file",
                    ),
                }
            }

            if fetched < SWEEP_BATCH || purged == 0 {
                if purged == 0 && fetched == SWEEP_BATCH {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        sweep = sweep.label(),
                        fetched,
                        "Sweep made no progress on a full batch; stopping until next tick",
                    );
                }
                break;
            }
        }
        Ok(())
    }

    /// Runs the shared teardown for one file: soft-delete the row (a no-op if
    /// already deleted, as in the reconcile sweep), purge its object, and stamp
    /// `purged_at`. Reports whether the object was actually reclaimed. The same
    /// path the manual file-delete handler takes.
    async fn purge_file(&self, file: &ExpiredFileRef) -> Result<PurgeOutcome> {
        let mut conn = self.infra.postgres.get_connection().await?;
        self.blob
            .purge_file(&mut conn, file.id, &file.storage_path, &file.storage_bucket)
            .await
    }
}

/// Which set of files a sweep reclaims.
#[derive(Debug, Clone, Copy)]
enum Sweep {
    /// Live files past their retention window.
    Expire,
    /// Soft-deleted files whose object was never reclaimed.
    Reconcile,
}

impl Sweep {
    /// Short label for structured logs.
    fn label(self) -> &'static str {
        match self {
            Sweep::Expire => "expire",
            Sweep::Reconcile => "reconcile",
        }
    }
}
