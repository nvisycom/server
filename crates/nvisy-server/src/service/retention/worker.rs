//! Data-retention worker: periodically expires files whose retention window has
//! elapsed.
//!
//! Every stored object that retention governs — original documents, redacted
//! outputs, and audit blobs — is a `workspace_files` row carrying a precomputed
//! `expires_at` (set at write time from the effective retention rule for its
//! kind). The worker is therefore a single indexed sweep: find live files with
//! `expires_at < now()`, soft-delete each row, and purge its backing object from
//! the bucket named on the row.

use std::str::FromStr;
use std::time::Duration;

use nvisy_nats::NatsClient;
use nvisy_nats::object::{AuditBucket, AuditKey, FileKey, FilesBucket, ObjectBucket};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::{WorkspaceFileRepository, WorkspacePipelineRunRepository};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::handler::Result;

/// Tracing target for the retention worker.
const TRACING_TARGET: &str = "nvisy_server::worker::retention";

/// How often the retention sweep runs. Retention is day-granular, so an hourly
/// sweep is responsive without being costly.
const TICK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Maximum files expired per sweep, bounding the work per tick.
const SWEEP_BATCH: i64 = 500;

/// Periodically expires stored files per their precomputed `expires_at`.
pub struct RetentionWorker {
    postgres: PgClient,
    nats: NatsClient,
}

impl RetentionWorker {
    /// Creates a new [`RetentionWorker`].
    pub fn new(postgres: PgClient, nats: NatsClient) -> Self {
        Self { postgres, nats }
    }

    /// Runs the worker until cancelled, logging its lifecycle.
    pub async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting retention worker");

        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = ticker.tick() => {
                    if let Err(err) = self.sweep().await {
                        tracing::error!(target: TRACING_TARGET, error = %err, "Retention sweep failed");
                    }
                }
            }
        }

        tracing::info!(target: TRACING_TARGET, "Retention worker stopped");
        Ok(())
    }

    /// Expires all due files, in `SWEEP_BATCH`-sized pages.
    ///
    /// A failed expiry leaves the file's `deleted_at` unset, so the same row is
    /// re-selected by the next page's query. The loop therefore advances on
    /// *successful* expirations, not rows fetched: it stops once a page is short
    /// (nothing more is due) or a full page made no progress (every file in it
    /// failed), so a persistently failing batch is logged once per tick instead
    /// of spinning forever.
    async fn sweep(&self) -> Result<()> {
        loop {
            let due = {
                let mut conn = self.postgres.get_connection().await?;
                conn.files_due_for_expiry(SWEEP_BATCH).await?
            };

            let fetched = due.len() as i64;
            let mut expired = 0i64;
            for file in due {
                match self
                    .expire_file(file.id, &file.storage_path, &file.storage_bucket)
                    .await
                {
                    Ok(()) => expired += 1,
                    Err(err) => tracing::error!(
                        target: TRACING_TARGET,
                        file_id = %file.id,
                        error = %err,
                        "Failed to expire file",
                    ),
                }
            }

            // Short page: nothing more is due. Zero progress on a full page: the
            // remaining rows keep failing, so stop rather than re-fetch them.
            if fetched < SWEEP_BATCH || expired == 0 {
                if expired == 0 && fetched == SWEEP_BATCH {
                    tracing::warn!(
                        target: TRACING_TARGET,
                        fetched,
                        "Retention sweep made no progress on a full batch; stopping until next tick",
                    );
                }
                break;
            }
        }
        Ok(())
    }

    /// Expires one file: clear any pipeline-run references to it, soft-delete the
    /// row (stops reads), and remove its backing object. The run's
    /// `audit_file_id`/`output_file_id` FKs are `ON DELETE SET NULL`, but that
    /// fires only on a hard delete; the soft delete keeps the row, so runs are
    /// nulled out explicitly here to avoid dangling references. Object removal is
    /// best-effort — a tombstoned row with an orphaned object is only logged.
    async fn expire_file(
        &self,
        file_id: Uuid,
        storage_path: &str,
        storage_bucket: &str,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        conn.clear_run_file_references(file_id).await?;
        conn.delete_workspace_file(file_id).await?;
        drop(conn);

        if let Err(err) = self.delete_object(storage_bucket, storage_path).await {
            tracing::error!(
                target: TRACING_TARGET,
                error = %err,
                "Failed to delete expired object",
            );
        }
        Ok(())
    }

    /// Removes an object from whichever bucket its row names.
    async fn delete_object(&self, bucket: &str, storage_path: &str) -> Result<()> {
        match bucket {
            b if b == FilesBucket::NAME => {
                if let Ok(key) = FileKey::from_str(storage_path) {
                    self.nats
                        .object_store::<FilesBucket>()
                        .await?
                        .delete(&key)
                        .await?;
                }
            }
            b if b == AuditBucket::NAME => {
                if let Ok(key) = AuditKey::from_str(storage_path) {
                    self.nats
                        .object_store::<AuditBucket>()
                        .await?
                        .delete(&key)
                        .await?;
                }
            }
            other => {
                tracing::warn!(
                    target: TRACING_TARGET,
                    bucket = %other,
                    "Expired file references an unknown bucket; object not removed",
                );
            }
        }
        Ok(())
    }
}
