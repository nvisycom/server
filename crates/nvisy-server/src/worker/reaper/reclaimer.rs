//! Blob object reclamation for the reaper's GC path.

use nvisy_postgres::PgConn;
use nvisy_postgres::query::{ReclaimableBlob, WorkspaceBlobRepository};

use crate::response::Result;
use crate::service::{Infra, delete_object};

/// Tracing target for blob-reclamation operations.
const TRACING_TARGET: &str = "nvisy_server::worker::reaper";

/// Whether a reclaim step removed a blob's backing object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a Pending reclaim is not progress and must not be counted as one"]
pub(super) enum PurgeOutcome {
    /// The object was removed and `reclaimed_at` was stamped.
    Purged,
    /// The object was not reclaimed — the blob gained a reference before it could
    /// be claimed, or the delete failed (store failure, bad key, or unknown
    /// bucket). A claimed-but-not-reclaimed blob is retried by the reconcile sweep.
    Pending,
}

/// Reclaims a run's blob objects in the first-party blob store — the reaper's
/// GC-side counterpart to the [`ArtifactWriter`]/[`ArtifactReader`] request-path
/// services.
///
/// It holds only the shared [`Infra`] clients (all `Arc`-backed) and takes the
/// per-sweep database connection as a method argument. Unlike that pair it needs
/// no crypto — object deletion is content-agnostic — so the reaper depends on the
/// object store alone, not the workspace keys.
///
/// [`ArtifactWriter`]: crate::service::ArtifactWriter
/// [`ArtifactReader`]: crate::service::ArtifactReader
pub(super) struct BlobReclaimer {
    infra: Infra,
}

impl BlobReclaimer {
    /// Creates a new [`BlobReclaimer`] over the internal object store.
    pub(super) fn new(infra: Infra) -> Self {
        Self { infra }
    }

    /// Claims a due blob and reclaims its object.
    ///
    /// The claim (`purged_at`) is committed *before* the object is deleted, so the
    /// blob leaves the dedup set the instant it is claimed: `find_or_create_blob`
    /// can never hand out a reference to bytes that are about to be — or already —
    /// gone. If a reference was acquired since the sweep read the blob, the claim
    /// matches no row and the blob is skipped ([`PurgeOutcome::Pending`]). A claim
    /// whose object delete then fails stays claimed for the reconcile sweep to
    /// retry, so a transient store outage self-heals without ever resurrecting the
    /// bytes.
    ///
    /// # Errors
    ///
    /// - A database error if the claim query fails. Once the blob is claimed, the
    ///   object reclaim is folded into the returned [`PurgeOutcome`] (a failed
    ///   delete yields `Pending`, not an error).
    pub(super) async fn purge_blob(
        &self,
        conn: &mut PgConn,
        blob: &ReclaimableBlob,
    ) -> Result<PurgeOutcome> {
        // Claim first, in its own committed step: once purged_at is set the blob no
        // longer deduplicates, so deleting its object next cannot strand a live
        // reference even if this process crashes before the delete.
        if conn.claim_blob_for_purge(blob.id).await?.is_none() {
            return Ok(PurgeOutcome::Pending);
        }
        Ok(self.reclaim_claimed_object(conn, blob).await)
    }

    /// Reclaims the object of a blob already claimed for purge (`purged_at` set),
    /// stamping `reclaimed_at` on success. Backs the reconcile sweep's retries.
    ///
    /// A failed delete leaves `reclaimed_at` NULL so the blob is retried; the blob
    /// is already out of the dedup set, so its bytes are never reused meanwhile.
    pub(super) async fn reclaim_claimed_object(
        &self,
        conn: &mut PgConn,
        blob: &ReclaimableBlob,
    ) -> PurgeOutcome {
        if let Err(err) =
            delete_object(&self.infra.blobs, &blob.storage_bucket, &blob.storage_path).await
        {
            tracing::error!(
                target: TRACING_TARGET,
                blob_id = %blob.id,
                error = %err,
                "Failed to delete claimed blob object; left for the reaper to retry",
            );
            return PurgeOutcome::Pending;
        }

        if let Err(err) = conn.mark_blob_reclaimed(blob.id).await {
            tracing::error!(
                target: TRACING_TARGET,
                blob_id = %blob.id,
                error = %err,
                "Deleted blob object but failed to mark it reclaimed; will retry",
            );
            return PurgeOutcome::Pending;
        }
        PurgeOutcome::Purged
    }
}
