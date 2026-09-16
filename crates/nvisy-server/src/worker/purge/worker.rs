//! The workspace purge worker: its tick loop and the two teardown steps.

use std::time::Duration;

use nvisy_postgres::query::WorkspaceRepository;
use tokio_util::sync::CancellationToken;

use crate::response::Result;
use crate::service::Infra;
use crate::worker::Worker;
use crate::worker::purge::PurgeConfig;

/// Tracing target for the workspace purge worker.
const TRACING_TARGET: &str = "nvisy_server::worker::purge";

/// How often the purge worker runs. The grace window is day-scale, so a daily
/// pass is timely without being costly.
const TICK_INTERVAL: Duration = Duration::from_hours(6);

/// Maximum workspaces advanced through the purge per tick, bounding the work.
const PURGE_BATCH: i64 = 50;

/// Periodically tears down workspaces whose soft-delete grace window has elapsed.
pub struct WorkspacePurgeWorker {
    infra: Infra,
    config: PurgeConfig,
}

impl Worker for WorkspacePurgeWorker {
    type Output = Result<()>;

    fn name(&self) -> &'static str {
        "workspace_purge"
    }

    /// Runs the purge worker until cancelled, logging its lifecycle.
    async fn run(&self, cancel: CancellationToken) -> Result<()> {
        tracing::info!(target: TRACING_TARGET, "Starting workspace purge worker");

        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                _ = ticker.tick() => {
                    if let Err(err) = self.tick(&cancel).await {
                        tracing::error!(target: TRACING_TARGET, error = %err, "Workspace purge tick failed");
                    }
                }
            }
        }

        tracing::info!(target: TRACING_TARGET, "Workspace purge worker stopped");
        Ok(())
    }
}

impl WorkspacePurgeWorker {
    /// Creates a new [`WorkspacePurgeWorker`].
    #[must_use]
    pub fn new(infra: Infra, config: PurgeConfig) -> Self {
        Self { infra, config }
    }

    /// One purge tick: for each workspace past its grace window, release the blob
    /// references its entities hold and, once its blobs have been reclaimed,
    /// hard-delete it. Both steps are idempotent, so a workspace whose blobs are
    /// not yet reclaimed is simply revisited on a later tick.
    async fn tick(&self, cancel: &CancellationToken) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        let due = conn
            .list_workspaces_pending_purge(self.config.grace, PURGE_BATCH)
            .await?;

        for workspace_id in due {
            if cancel.is_cancelled() {
                break;
            }

            // Release the references the workspace's entities hold and expire the
            // freed blobs, so the reaper reclaims their bytes (even blobs kept
            // indefinitely). Idempotent across ticks.
            conn.release_and_expire_workspace_blobs(workspace_id)
                .await?;

            // Hard-delete only once every blob is gone, so the cascade never strands
            // an object; otherwise leave the workspace for a later tick.
            if conn.workspace_blobs_reclaimed(workspace_id).await? {
                conn.purge_workspace(workspace_id).await?;
                tracing::info!(
                    target: TRACING_TARGET,
                    %workspace_id,
                    "Purged workspace past its grace window",
                );
            }
        }
        Ok(())
    }
}
