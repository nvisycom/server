//! Connection sync transfer engine: runs a sync transfer to completion and
//! records the run's outcome.
//!
//! [`TransferEngine`] owns the transfer mechanics — racing the transfer against a
//! timeout and a caller-supplied cancel signal, and finalizing the run and its
//! terminal event. It dispatches each [`TransferKind`] to the direction
//! collaborator that performs it: the importer (`import` module) pulls files in,
//! the exporter (`export` module) pushes them out. Both stream end to end and keep
//! files encrypted at rest.
//!
//! The request-time [`ConnectionSyncService`] and the scheduled
//! [`ConnectionSyncWorker`] both drive a transfer through this engine; the
//! process-local cancel registry lives on the service handle, which hands the
//! engine the token to observe.
//!
//! [`ConnectionSyncService`]: crate::service::ConnectionSyncService
//! [`ConnectionSyncWorker`]: super::ConnectionSyncWorker

use nvisy_file_service::FileService;
use nvisy_postgres::AsyncConnection;
use nvisy_postgres::query::WorkspaceConnectionSyncRepository;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::connector::Connector;
use super::export::Exporter;
use super::import::Importer;
use crate::extract::SecurityContext;
use crate::response::{ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{
    CryptoService, ExternalObjectStore, Infra, TransferKind, TransferRequest, event,
};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::worker::integration";

/// Maximum wall-clock time for a single sync transfer before it is failed.
const SYNC_TIMEOUT: std::time::Duration = std::time::Duration::from_mins(30);

/// How a transfer ended: either it ran to a result/timeout, or it was cancelled.
enum Outcome {
    Finished(Result<u64>),
    Cancelled,
}

/// Runs sync transfers between an external connection and the internal file store,
/// dispatching each [`TransferKind`] to the importer or exporter that performs it.
///
/// Cheaply cloneable (every field is `Arc`-backed).
#[derive(Clone)]
#[must_use = "engine does nothing unless you run a transfer with it"]
pub struct TransferEngine {
    infra: Infra,
    importer: Importer,
    exporter: Exporter,
}

impl TransferEngine {
    /// Creates a new [`TransferEngine`]. `import_concurrency` and
    /// `export_concurrency` bound the in-flight imports and exports per sync (see
    /// [`IntegrationConfig`]).
    ///
    /// [`IntegrationConfig`]: crate::service::IntegrationConfig
    pub fn new(
        infra: Infra,
        crypto: CryptoService,
        object: ExternalObjectStore,
        cloud: FileService,
        import_concurrency: usize,
        export_concurrency: usize,
    ) -> Self {
        let connector = Connector::new(infra.clone(), crypto.clone(), object, cloud);
        let importer = Importer::new(
            infra.clone(),
            crypto.clone(),
            connector.clone(),
            import_concurrency,
        );
        let exporter = Exporter::new(infra.clone(), crypto, connector, export_concurrency);
        Self {
            infra,
            importer,
            exporter,
        }
    }

    /// Runs a sync transfer to completion and records the run's outcome.
    ///
    /// The transfer runs in an inner task bounded by a fixed timeout: a panic
    /// surfaces as a join error and a hung backend as a timeout, both recorded as a
    /// failed run rather than leaving it stuck `Running`. The request's
    /// [`TransferKind`] selects the direction and scope. Cancelling `token` (from
    /// the service handle's registry) aborts the transfer and records the run as
    /// cancelled. Shared by the manual endpoint and the scheduled worker.
    pub async fn run_transfer(&self, request: TransferRequest, token: CancellationToken) {
        // Copy the scalar identifiers needed after the transfer task takes
        // ownership of the request below (for the terminal event).
        let run_id = request.run_id;
        let account_id = request.account_id;
        let workspace_id = request.connection.workspace_id;
        let connection_id = request.connection.id;
        let connection_name = request.connection.display_name.clone();

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
            () = token.cancelled() => {
                work.abort();
                Outcome::Cancelled
            }
            () = tokio::time::sleep(SYNC_TIMEOUT) => {
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

        match outcome {
            Outcome::Finished(result) => {
                let origin = event::EventOrigin {
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
        origin: event::EventOrigin<'_>,
        connection_id: Uuid,
        connection_name: &str,
        result: Result<u64>,
    ) {
        // Build the terminal event before `result` is consumed. `notify` targets
        // the account the run is attributed to (the origin's).
        let event = match &result {
            Ok(records_synced) => {
                event::WorkspaceEvent::ConnectionSyncCompleted(event::ConnectionSyncCompleted {
                    connection_id,
                    connection_name: connection_name.to_owned(),
                    records_synced: Some(i64::try_from(*records_synced).unwrap_or(i64::MAX)),
                    notify: Some(origin.account_id),
                })
            }
            Err(err) => {
                // Log the full error (may include backend URLs/details) but record
                // only the safe summary; the stored message is exposed to clients.
                tracing::warn!(target: TRACING_TARGET, %run_id, error = %err, "Sync failed");
                event::WorkspaceEvent::ConnectionSyncFailed(event::ConnectionSyncFailed {
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
                let transitioned = if let Ok(records_synced) = &result {
                    conn.complete_workspace_connection_sync(
                        run_id,
                        i64::try_from(*records_synced).unwrap_or(i64::MAX),
                    )
                    .await?
                    .is_some()
                } else {
                    let safe_message = match &event {
                        event::WorkspaceEvent::ConnectionSyncFailed(e) => {
                            e.error.clone().unwrap_or_else(|| "Sync failed".to_owned())
                        }
                        _ => "Sync failed".to_owned(),
                    };
                    conn.fail_workspace_connection_sync(run_id, &safe_message)
                        .await?
                        .is_some()
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
