//! Connection sync domain service: starting, listing, reading, and cancelling a
//! connection's sync runs.

use nvisy_postgres::model::{
    NewWorkspaceConnectionSync, WorkspaceConnection, WorkspaceConnectionSync,
};
use nvisy_postgres::query::{
    WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository,
    WorkspaceConnectionSyncRepository,
};
use nvisy_postgres::types::{
    CursorPage, CursorPagination, SyncDeletionPolicy, SyncMode, SyncStatus, SyncTriggerType,
    TimeCursor, WithAccountRef,
};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use crate::response::{Error, ErrorKind, Resource, Result};
use crate::service::{
    ConnectionConfig, ConnectionSyncService, CryptoService, TransferKind, TransferRequest, event,
};
use crate::worker::integration::SourceEntry;

/// Tracing target for connection sync domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::connection_syncs";

/// What an on-demand sync transfers.
#[derive(Debug, Clone)]
pub enum SyncRequest {
    /// The object-store connection's configured direction: import every new
    /// object, or export every redacted output not yet exported.
    Configured,
    /// The files selected in a file service's picker.
    ImportSelected(Vec<SourceEntry>),
    /// The given workspace files, each exported as a new provider file.
    ExportSelected(Vec<Uuid>),
}

/// Starts, lists, reads, and cancels a connection's sync runs.
#[derive(Clone)]
pub struct WorkspaceConnectionSyncService {
    postgres: PgClient,
    crypto: CryptoService,
    sync: ConnectionSyncService,
}

impl WorkspaceConnectionSyncService {
    /// Creates a [`WorkspaceConnectionSyncService`] over its clients.
    #[must_use]
    pub fn new(postgres: PgClient, crypto: CryptoService, sync: ConnectionSyncService) -> Self {
        Self {
            postgres,
            crypto,
            sync,
        }
    }

    /// Starts an on-demand sync run on a connection, recording its start event,
    /// and spawns the transfer. Returns the run as created; poll it for
    /// completion.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the connection does not exist in the workspace.
    /// - `BadRequest` if the connection is inactive, or the request does not fit
    ///   its kind (a configured sync on a file service, a picker import on an
    ///   object store).
    /// - `Conflict` if a sync is already in progress.
    /// - A crypto or database error otherwise.
    pub async fn start(
        &self,
        origin: event::EventOrigin<'_>,
        connection_id: Uuid,
        request: SyncRequest,
    ) -> Result<WorkspaceConnectionSync> {
        let mut conn = self.postgres.get_connection().await?;
        let (connection, config) = self
            .load_syncable(&mut conn, origin.workspace_id, connection_id)
            .await?;
        let kind = match request {
            SyncRequest::Configured => {
                if !config.supports_schedule() {
                    return Err(ErrorKind::BadRequest.with_message(
                        "This connection has no whole-listing sync; file services import via \
                         the picker and export per file",
                    ));
                }
                let schedule = conn.find_connection_schedule(connection.id).await?;
                let sync_mode = schedule.as_ref().map_or(SyncMode::Import, |s| s.sync_mode);
                let deletion_policy = schedule
                    .as_ref()
                    .map_or(SyncDeletionPolicy::Ignore, |s| s.deletion_policy);
                match sync_mode {
                    SyncMode::Import => TransferKind::ImportAll { deletion_policy },
                    SyncMode::Export => TransferKind::ExportRedacted,
                }
            }
            SyncRequest::ImportSelected(entries) => {
                if !config.is_file_service() {
                    return Err(ErrorKind::BadRequest
                        .with_message("Picker import is only available for file services"));
                }
                TransferKind::ImportSelected { entries }
            }
            SyncRequest::ExportSelected(file_ids) => TransferKind::ExportSelected { file_ids },
        };

        let new_run = NewWorkspaceConnectionSync {
            connection_id: connection.id,
            account_id: origin.account_id,
            trigger_type: Some(SyncTriggerType::OnDemand),
            status: Some(SyncStatus::Running),
            records_synced: Some(0),
            attempt: Some(1),
            metadata: None,
        };
        let run = self
            .sync
            .create_run(&mut conn, origin, new_run, &connection)
            .await?;
        drop(conn);

        let sync = self.sync.clone();
        let request = TransferRequest {
            run_id: run.id,
            connection,
            config,
            account_id: origin.account_id,
            kind,
        };
        tokio::spawn(async move { sync.run_transfer(request).await });

        tracing::info!(target: TRACING_TARGET, sync_id = %run.id, "Connection sync started");
        Ok(run)
    }

    /// Lists a connection's sync runs, newest first, each with its trigger.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the connection does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn list_for_connection(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspaceConnectionSync>>> {
        let mut conn = self.postgres.get_connection().await?;
        let connection = find_connection(&mut conn, workspace_id, connection_id).await?;
        Ok(conn
            .cursor_list_workspace_connection_syncs(connection.id, pagination, None)
            .await?)
    }

    /// Lists every sync run across a workspace's connections, newest first,
    /// optionally filtered by status and by connection provider.
    ///
    /// # Errors
    ///
    /// - A database error if the query fails.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
        status: Option<SyncStatus>,
        providers: &[String],
    ) -> Result<CursorPage<WithAccountRef<WorkspaceConnectionSync>>> {
        let mut conn = self.postgres.get_connection().await?;
        let page = conn
            .cursor_list_workspace_connection_syncs_all(workspace_id, pagination, status, providers)
            .await?;
        Ok(page.map(|(run, _connection_id)| run))
    }

    /// Finds a connection's sync run, with its trigger.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the connection or the run does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        sync_id: Uuid,
    ) -> Result<WithAccountRef<WorkspaceConnectionSync>> {
        let mut conn = self.postgres.get_connection().await?;
        find_run(&mut conn, workspace_id, connection_id, sync_id).await
    }

    /// Cancels an in-progress sync run: flips its status, then signals the
    /// transfer if it runs on this instance. A run on another instance is stopped
    /// by the status flip plus the status-guarded finalizers.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the connection or the run does not exist in the workspace.
    /// - `Conflict` if the run is not in progress.
    /// - A database error if the query fails.
    pub async fn cancel(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        sync_id: Uuid,
    ) -> Result<WithAccountRef<WorkspaceConnectionSync>> {
        let mut conn = self.postgres.get_connection().await?;
        let run = find_run(&mut conn, workspace_id, connection_id, sync_id).await?;
        let cancelled = conn
            .cancel_workspace_connection_sync(run.item.id)
            .await?
            .ok_or_else(|| {
                ErrorKind::Conflict.with_message("Sync is not in progress and cannot be cancelled")
            })?;
        let cancelled_locally = self.sync.cancel_local(run.item.id);

        tracing::info!(
            target: TRACING_TARGET,
            sync_id = %cancelled.id,
            cancelled_locally,
            "Connection sync cancelled",
        );
        // Cancelling does not change who triggered the run.
        Ok(WithAccountRef {
            item: cancelled,
            account: run.account,
        })
    }

    /// Loads an active connection with no sync in progress, and decrypts its
    /// configuration.
    async fn load_syncable(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        connection_id: Uuid,
    ) -> Result<(WorkspaceConnection, ConnectionConfig)> {
        let connection = find_connection(conn, workspace_id, connection_id).await?;
        if !connection.is_active {
            return Err(ErrorKind::BadRequest.with_message("Connection is not active"));
        }
        if let Some(latest) = conn
            .find_latest_workspace_connection_sync(connection.id)
            .await?
            && latest.status.is_in_progress()
        {
            return Err(ErrorKind::Conflict.with_message("A sync is already in progress"));
        }
        let config = self
            .crypto
            .decrypt_json(workspace_id, &connection.encrypted_data)?;
        Ok((connection, config))
    }
}

/// Loads a connection scoped to the workspace, or a `NotFound`.
async fn find_connection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_id: Uuid,
) -> Result<WorkspaceConnection> {
    conn.find_connection_in_workspace(workspace_id, connection_id)
        .await?
        .ok_or_else(|| Error::not_found(Resource::Connection))
}

/// Loads a connection's sync run with its trigger, or a `NotFound`. The run must
/// belong to the addressed connection, not just the workspace.
async fn find_run(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_id: Uuid,
    sync_id: Uuid,
) -> Result<WithAccountRef<WorkspaceConnectionSync>> {
    let connection = find_connection(conn, workspace_id, connection_id).await?;
    conn.find_connection_sync_in_workspace(workspace_id, sync_id)
        .await?
        .filter(|run| run.item.connection_id == connection.id)
        .ok_or_else(|| Error::not_found(Resource::ConnectionSync))
}
