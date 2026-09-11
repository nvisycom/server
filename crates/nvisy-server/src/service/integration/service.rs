//! Connection sync service: the run lifecycle that drives file transfers between
//! a workspace's external connection and the first-party blob store.
//!
//! [`ConnectionSyncService`] owns the sync-run bookkeeping — opening a run,
//! racing the transfer against a timeout and a cancel signal, and finalizing the
//! run and its terminal event. The transfer mechanics live in the direction
//! collaborators it dispatches to: the importer (`import` module) pulls files in,
//! the exporter (`export` module) pushes them out. Both stream end to end and
//! keep files encrypted at rest.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nvisy_file_service::FileService;
use nvisy_postgres::model::{
    NewWorkspaceConnectionSync, WorkspaceConnection, WorkspaceConnectionSync,
};
use nvisy_postgres::query::WorkspaceConnectionSyncRepository;
use nvisy_postgres::types::SyncDeletionPolicy;
use nvisy_postgres::{AsyncConnection, PgConn};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::connector::Connector;
use super::export::Exporter;
use super::file_source::SourceEntry;
use super::import::Importer;
use crate::extract::SecurityContext;
use crate::response::{ErrorKind, Result};
use crate::service::{
    ConnectionConfig, ConnectionSyncCompleted, ConnectionSyncFailed, ConnectionSyncStarted,
    EventEmitter, EventOrigin, ExternalObjectStore, Infra, WorkspaceEvent,
};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::service::sync";

/// Maximum wall-clock time for a single sync transfer before it is failed.
const SYNC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// How a transfer ended: either it ran to a result/timeout, or it was cancelled.
enum Outcome {
    Finished(Result<u64>),
    Cancelled,
}

/// What a transfer moves, and in which direction.
pub enum TransferKind {
    /// Import every not-yet-imported entry from the connection's listing,
    /// reconciling source deletions per `deletion_policy`. Used by scheduled
    /// object-store syncs.
    ImportAll {
        /// How to reconcile entries that vanished from the source.
        deletion_policy: SyncDeletionPolicy,
    },
    /// Import a caller-selected set of provider files, skipping the listing.
    /// Used by the file-service picker; already-imported ids are skipped. Each
    /// entry carries the provider id and the display name the picker returned.
    ImportSelected {
        /// The provider files to import (id + name), as returned by the picker.
        entries: Vec<SourceEntry>,
    },
    /// Export a caller-selected set of stored workspace files to the connection,
    /// each as a new provider file. Used by the manual export endpoint.
    ExportSelected {
        /// The workspace files to export, by id.
        file_ids: Vec<Uuid>,
    },
    /// Export every redacted output in the connection's workspace that has not
    /// yet been exported to this connection. Used by scheduled export syncs.
    ExportRedacted,
}

/// The inputs to [`ConnectionSyncService::run_transfer`]: the run to execute, the
/// connection and its decrypted config, and what to move.
pub struct TransferRequest {
    /// The persisted run this transfer executes.
    pub run_id: Uuid,
    /// The connection being synced.
    pub connection: WorkspaceConnection,
    /// The connection's decrypted config, used to connect the file source.
    pub config: ConnectionConfig,
    /// The account the run is attributed to.
    pub account_id: Uuid,
    /// What the transfer moves, and in which direction.
    pub kind: TransferKind,
}

/// Drives sync runs between an external connection and the internal file store.
///
/// Holds the run lifecycle (and the process-local cancel registry) and dispatches
/// each [`TransferKind`] to the importer or exporter that performs it.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct ConnectionSyncService {
    infra: Infra,
    importer: Importer,
    exporter: Exporter,
    // Cancellation tokens for transfers running in this process, keyed by run id.
    // Cancellation is best-effort and process-local: it aborts a transfer only on
    // the instance running it. Cross-instance runs are stopped by the DB status
    // flip plus the status-guarded finalizers.
    running: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
}

impl ConnectionSyncService {
    /// Creates a new [`ConnectionSyncService`]. `import_concurrency` and
    /// `export_concurrency` bound the in-flight imports and exports per sync (see
    /// [`IntegrationConfig`](crate::service::IntegrationConfig)).
    pub fn new(
        infra: Infra,
        object: ExternalObjectStore,
        cloud: FileService,
        import_concurrency: usize,
        export_concurrency: usize,
    ) -> Self {
        let connector = Connector::new(infra.clone(), object, cloud);
        let importer = Importer::new(infra.clone(), connector.clone(), import_concurrency);
        let exporter = Exporter::new(infra.clone(), connector, export_concurrency);
        Self {
            infra,
            importer,
            exporter,
            running: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Creates a sync run row and records its `ConnectionSyncStarted` event in one
    /// transaction on the caller's connection, so the run and its start event
    /// commit together — the start event can never be lost for a run that was
    /// created. Returns the persisted run.
    pub async fn create_run(
        &self,
        conn: &mut PgConn,
        new_run: NewWorkspaceConnectionSync,
        connection: &WorkspaceConnection,
    ) -> Result<WorkspaceConnectionSync> {
        let origin = EventOrigin {
            workspace_id: connection.workspace_id,
            account_id: new_run.account_id,
            security: &SecurityContext::default(),
        };
        let started = WorkspaceEvent::ConnectionSyncStarted(ConnectionSyncStarted {
            connection_id: connection.id,
            connection_name: connection.display_name.clone(),
        });
        let run = conn
            .transaction(async |conn| {
                let run = conn.create_workspace_connection_sync(new_run).await?;
                conn.emit_event(origin, started).await?;
                Ok::<_, crate::response::Error>(run)
            })
            .await?;
        Ok(run)
    }

    /// Signals a locally-running transfer to stop, if this instance is running
    /// it. Returns whether a token was found and cancelled. This is best-effort:
    /// a run executing on another instance is not reached here and relies on the
    /// DB status flip instead.
    pub fn cancel_local(&self, run_id: Uuid) -> bool {
        let guard = self.running.lock().expect("sync cancel registry poisoned");
        if let Some(token) = guard.get(&run_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Runs a sync transfer to completion and records the run's outcome.
    ///
    /// The transfer runs in an inner task bounded by a fixed timeout: a panic
    /// surfaces as a join error and a hung backend as a timeout, both recorded
    /// as a failed run rather than leaving it stuck `Running`. The request's
    /// [`TransferKind`] selects the direction and scope. A cancel signal (see
    /// [`cancel_local`]) aborts the transfer and records the run as cancelled.
    /// Shared by the manual endpoint and the scheduled worker.
    ///
    /// [`cancel_local`]: Self::cancel_local
    pub async fn run_transfer(&self, request: TransferRequest) {
        // Copy the scalar identifiers needed after the transfer task takes
        // ownership of the request below (for the cancel registry and the
        // terminal event).
        let run_id = request.run_id;
        let account_id = request.account_id;
        let workspace_id = request.connection.workspace_id;
        let connection_id = request.connection.id;
        let connection_name = request.connection.display_name.clone();

        let token = CancellationToken::new();
        self.running
            .lock()
            .expect("sync cancel registry poisoned")
            .insert(run_id, token.clone());

        let transfer = self.clone();
        let mut work = tokio::spawn(async move {
            let TransferRequest {
                connection,
                config,
                kind,
                ..
            } = request;
            match kind {
                TransferKind::ImportAll { deletion_policy } => {
                    transfer
                        .importer
                        .import_new(&connection, &config, deletion_policy, account_id)
                        .await
                }
                TransferKind::ImportSelected { entries } => {
                    transfer
                        .importer
                        .import_selected(&connection, &config, account_id, entries)
                        .await
                }
                TransferKind::ExportSelected { file_ids } => {
                    transfer
                        .exporter
                        .export_selected(&connection, &config, file_ids)
                        .await
                }
                TransferKind::ExportRedacted => {
                    transfer
                        .exporter
                        .export_redacted(&connection, &config)
                        .await
                }
            }
        });

        // Race the transfer against a cancel signal and a hard timeout. The join
        // handle is polled by mutable reference so it can still be aborted in the
        // cancel/timeout branches.
        let outcome = tokio::select! {
            _ = token.cancelled() => {
                work.abort();
                Outcome::Cancelled
            }
            _ = tokio::time::sleep(SYNC_TIMEOUT) => {
                work.abort();
                Outcome::Finished(Err(ErrorKind::InternalServerError.with_message("Sync timed out")))
            }
            joined = &mut work => match joined {
                Ok(result) => Outcome::Finished(result),
                Err(join_err) => Outcome::Finished(Err(ErrorKind::InternalServerError
                    .with_message("Sync task terminated unexpectedly")
                    .with_context(join_err.to_string()))),
            },
        };

        self.running
            .lock()
            .expect("sync cancel registry poisoned")
            .remove(&run_id);

        match outcome {
            Outcome::Finished(result) => {
                let origin = EventOrigin {
                    workspace_id,
                    account_id,
                    security: &SecurityContext::default(),
                };
                self.finish_run(run_id, origin, connection_id, &connection_name, result)
                    .await;
            }
            Outcome::Cancelled => self.cancel_run(run_id).await,
        }
    }

    /// Finalizes a background sync run and records its terminal event atomically.
    ///
    /// The status-guarded finalize and the outbox event commit in one transaction,
    /// and the event is recorded only when the finalize actually transitioned the
    /// run (its guarded update matched a row). So a run that a cancellation or reap
    /// already moved to a terminal state produces no spurious completed/failed
    /// event, and a finalized run never lacks its event. Errors are logged rather
    /// than propagated, since this runs after the response was already sent.
    async fn finish_run(
        &self,
        run_id: Uuid,
        origin: EventOrigin<'_>,
        connection_id: Uuid,
        connection_name: &str,
        result: Result<u64>,
    ) {
        // Build the terminal event before `result` is consumed. `notify` targets
        // the account the run is attributed to (the origin's).
        let event = match &result {
            Ok(records_synced) => {
                WorkspaceEvent::ConnectionSyncCompleted(ConnectionSyncCompleted {
                    connection_id,
                    connection_name: connection_name.to_owned(),
                    records_synced: Some(*records_synced as i64),
                    notify: Some(origin.account_id),
                })
            }
            Err(err) => {
                // Log the full error (may include backend URLs/details) but record
                // only the safe summary; the stored message is exposed to clients.
                tracing::warn!(target: TRACING_TARGET, %run_id, error = %err, "Sync failed");
                WorkspaceEvent::ConnectionSyncFailed(ConnectionSyncFailed {
                    connection_id,
                    connection_name: connection_name.to_owned(),
                    error: Some(err.message.as_deref().unwrap_or("Sync failed").to_owned()),
                    notify: Some(origin.account_id),
                })
            }
        };

        let mut conn = match self.infra.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to record sync outcome: no connection");
                return;
            }
        };

        let finalized = conn
            .transaction(async |conn| {
                // The guarded update returns `Some` only if it transitioned an
                // active run; a run already terminal (cancelled/reaped) returns
                // `None`, and we record no event for it.
                let transitioned = match &result {
                    Ok(records_synced) => conn
                        .complete_workspace_connection_sync(run_id, *records_synced as i64)
                        .await?
                        .is_some(),
                    Err(_) => {
                        let safe_message = match &event {
                            WorkspaceEvent::ConnectionSyncFailed(e) => {
                                e.error.clone().unwrap_or_else(|| "Sync failed".to_owned())
                            }
                            _ => "Sync failed".to_owned(),
                        };
                        conn.fail_workspace_connection_sync(run_id, &safe_message)
                            .await?
                            .is_some()
                    }
                };
                if transitioned {
                    conn.emit_event(origin, event).await?;
                }
                Ok::<_, crate::response::Error>(transitioned)
            })
            .await;

        match finalized {
            Ok(false) => {
                tracing::debug!(target: TRACING_TARGET, %run_id, "Sync run already terminal; no event recorded");
            }
            Ok(true) => {}
            Err(err) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    %run_id, error = %err,
                    "Failed to finalize sync run",
                );
            }
        }
    }

    /// Marks a cancelled run's row as cancelled. The status transition is
    /// guarded, so if the cancel handler already flipped the row (or it reached
    /// another terminal state first) this is a harmless no-op.
    async fn cancel_run(&self, run_id: Uuid) {
        let mut conn = match self.infra.postgres.get_connection().await {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to record cancellation: no connection");
                return;
            }
        };
        if let Err(err) = conn.cancel_workspace_connection_sync(run_id).await {
            tracing::error!(target: TRACING_TARGET, %run_id, error = %err, "Failed to record sync cancellation");
        }
    }
}
