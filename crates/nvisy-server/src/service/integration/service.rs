//! Connection sync handle: the request-side lifecycle for file transfers between
//! a workspace's external connection and the first-party blob store.
//!
//! [`ConnectionSyncService`] owns the request-time surface — opening a run,
//! cancelling a locally-running one, and the process-local cancel registry that
//! ties the two together — and delegates the transfer itself to the
//! [`TransferEngine`](crate::worker::integration::TransferEngine), which races the
//! transfer against a timeout and finalizes the run. Both the manual endpoint and
//! the scheduled worker drive a transfer through this handle.

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

use crate::extract::SecurityContext;
use crate::response::Result;
use crate::service::event::EventEmitter;
use crate::service::{ConnectionConfig, CryptoService, ExternalObjectStore, Infra, event};
use crate::worker::integration::{SourceEntry, TransferEngine};

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

/// The inputs to a transfer: the run to execute, the connection and its decrypted
/// config, and what to move.
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

/// The request-side handle for connection sync runs.
///
/// Owns the run lifecycle (opening a run, the process-local cancel registry) and
/// delegates each transfer to the [`TransferEngine`], observing the registry's
/// per-run cancel token.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct ConnectionSyncService {
    engine: TransferEngine,
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
        crypto: CryptoService,
        object: ExternalObjectStore,
        cloud: FileService,
        import_concurrency: usize,
        export_concurrency: usize,
    ) -> Self {
        let engine = TransferEngine::new(
            infra,
            crypto,
            object,
            cloud,
            import_concurrency,
            export_concurrency,
        );
        Self {
            engine,
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
        let origin = event::EventOrigin {
            workspace_id: connection.workspace_id,
            account_id: new_run.account_id,
            security: &SecurityContext::default(),
        };
        let started = event::WorkspaceEvent::ConnectionSyncStarted(event::ConnectionSyncStarted {
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

    /// Runs a sync transfer to completion, registering it in the process-local
    /// cancel registry for the duration so [`cancel_local`](Self::cancel_local)
    /// can reach it, and delegating the transfer mechanics to the
    /// [`TransferEngine`]. Shared by the manual endpoint and the scheduled worker.
    pub async fn run_transfer(&self, request: TransferRequest) {
        let run_id = request.run_id;
        let token = CancellationToken::new();
        self.running
            .lock()
            .expect("sync cancel registry poisoned")
            .insert(run_id, token.clone());

        self.engine.run_transfer(request, token).await;

        self.running
            .lock()
            .expect("sync cancel registry poisoned")
            .remove(&run_id);
    }
}
