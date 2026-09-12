//! Blob reaper: reclaims stored objects once nothing references them.
//!
//! Every stored object — original documents, redacted outputs, audit blobs, and
//! intermediates — lives in a shared, ref-counted `workspace_blobs` row. A blob
//! becomes reclaimable only when its last referrer is gone (`ref_count = 0`),
//! which protects a blob shared by two identical uploads from being purged out
//! from under a still-live document. Retention is a time policy layered on top:
//! a blob is reclaimed once it is both unreferenced and past its window. Each
//! tick runs three stages:
//!
//! - **Referrer cleanup**: machine byproducts — audit rows and detection
//!   intermediates — hold a reference for their whole life, so their blobs never
//!   reach `ref_count = 0` on their own. This stage expires those referrers by
//!   their blob's retention window (deleting the audit row, nulling the
//!   intermediate pointer) and drops the reference, so an expired byproduct blob
//!   becomes reclaimable by the Expire sweep.
//! - **Expire**: unreferenced blobs whose retention window has elapsed
//!   (`ref_count = 0 AND expires_at < now()`, from the per-blob retention rule).
//!   Each is claimed ([`purged_at`], committed before any object delete, so it
//!   leaves the dedup set atomically) and then its object is reclaimed.
//! - **Reconcile**: blobs claimed for purge whose object delete was never
//!   confirmed (`purged_at IS NOT NULL AND reclaimed_at IS NULL`) — a delete that
//!   failed or a crash between claim and delete. The object delete is retried
//!   (idempotent) until `reclaimed_at` is stamped, so a transient object-store
//!   outage self-heals and a claimed blob's bytes are never reused meanwhile.
//!
//! [`purged_at`]: nvisy_postgres::model::Blob::purged_at

use std::time::Duration;

use nvisy_postgres::query::{
    WorkspaceAuditRepository, WorkspaceBlobRepository, WorkspaceDetectionRepository,
};
use tokio_util::sync::CancellationToken;

use crate::response::Result;
use crate::service::{CryptoService, Infra, PurgeOutcome, RunBlobStore, Worker};

/// Tracing target for the blob reaper.
const TRACING_TARGET: &str = "nvisy_server::worker::reaper";

/// How often the reaper runs. Retention is day-granular, so an hourly sweep is
/// responsive without being costly.
const TICK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Maximum blobs purged per sweep pass, bounding the work per tick.
const SWEEP_BATCH: i64 = 500;

/// Periodically reclaims expired blobs and reconciles orphaned objects.
pub struct BlobReaper {
    infra: Infra,
    blob: RunBlobStore,
}

impl Worker for BlobReaper {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "blob_reaper"
    }

    /// Runs the reaper until cancelled, logging its lifecycle.
    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting blob reaper");

        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => self.tick(&cancel).await,
            }
        }

        tracing::info!(target: TRACING_TARGET, "Blob reaper stopped");
        Ok(())
    }
}

impl BlobReaper {
    /// Creates a new [`BlobReaper`].
    pub fn new(infra: Infra, crypto: CryptoService) -> Self {
        let blob = RunBlobStore::new(infra.clone(), crypto);
        Self { infra, blob }
    }

    /// One reaper tick: release expired byproduct references, expire due blobs,
    /// then reconcile any orphaned objects. Referrer cleanup runs first so a
    /// byproduct blob it unreferences becomes reclaimable within the same tick;
    /// Reconcile runs last so it also catches any object a failed expiry left
    /// behind.
    async fn tick(&self, cancel: &CancellationToken) {
        if let Err(err) = self.release_expired_referrers(cancel).await {
            tracing::error!(target: TRACING_TARGET, error = %err, "Referrer cleanup failed");
        }
        if let Err(err) = self.sweep(Sweep::Expire, cancel).await {
            tracing::error!(target: TRACING_TARGET, error = %err, "Expiry sweep failed");
        }
        if let Err(err) = self.sweep(Sweep::Reconcile, cancel).await {
            tracing::error!(target: TRACING_TARGET, error = %err, "Reconcile sweep failed");
        }
    }

    /// Releases the references machine byproducts hold on their blobs once past
    /// retention: deletes expired audit rows and nulls expired detection
    /// intermediates, each dropping one blob reference. Pages through both until a
    /// pass clears nothing, so the following Expire sweep sees the now-unreferenced
    /// blobs. Stops early on cancellation so shutdown is not held up by a backlog.
    async fn release_expired_referrers(&self, cancel: &CancellationToken) -> Result<()> {
        loop {
            if cancel.is_cancelled() {
                break;
            }
            let mut conn = self.infra.postgres.get_connection().await?;
            let audits = conn.delete_expired_audits(SWEEP_BATCH).await?;
            let intermediates = conn.clear_expired_intermediates(SWEEP_BATCH).await?;
            if audits == 0 && intermediates == 0 {
                break;
            }
        }
        Ok(())
    }

    /// Reclaims a sweep's blobs in `SWEEP_BATCH`-sized pages.
    ///
    /// A confirmed reclaim takes the blob out of the sweep's result set (the
    /// Expire sweep by claiming it, the Reconcile sweep by stamping
    /// `reclaimed_at`), so the loop advances on *confirmed* reclaims rather than
    /// rows fetched: it stops once a page is short (nothing more due) or a full
    /// page made no progress (every blob in it failed), logging a stuck batch once
    /// per tick instead of spinning on it.
    async fn sweep(&self, sweep: Sweep, cancel: &CancellationToken) -> Result<()> {
        loop {
            if cancel.is_cancelled() {
                break;
            }
            let batch = {
                let mut conn = self.infra.postgres.get_connection().await?;
                match sweep {
                    Sweep::Expire => conn.blobs_due_for_purge(SWEEP_BATCH).await?,
                    Sweep::Reconcile => conn.blobs_pending_reclaim(SWEEP_BATCH).await?,
                }
            };

            let fetched = batch.len() as i64;
            let mut reclaimed = 0i64;
            for blob in batch {
                let mut conn = self.infra.postgres.get_connection().await?;
                let outcome = match sweep {
                    Sweep::Expire => self.blob.purge_blob(&mut conn, &blob).await,
                    Sweep::Reconcile => {
                        Ok(self.blob.reclaim_claimed_object(&mut conn, &blob).await)
                    }
                };
                match outcome {
                    // Only a confirmed reclaim is progress. A pending one leaves the
                    // blob in the sweep's set, so counting it would re-fetch the same
                    // batch forever.
                    Ok(PurgeOutcome::Purged) => reclaimed += 1,
                    Ok(PurgeOutcome::Pending) => {}
                    Err(err) => tracing::error!(
                        target: TRACING_TARGET,
                        sweep = sweep.label(),
                        blob_id = %blob.id,
                        error = %err,
                        "Failed to reclaim blob",
                    ),
                }
            }

            if fetched < SWEEP_BATCH || reclaimed == 0 {
                if reclaimed == 0 && fetched == SWEEP_BATCH {
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
}

/// Which set of blobs a sweep reclaims.
#[derive(Debug, Clone, Copy)]
enum Sweep {
    /// Unreferenced blobs past their retention window.
    Expire,
    /// Blobs claimed for purge whose object delete was never confirmed.
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
