//! Workspace connection domain logic: create, read, list, update, delete.
//!
//! Holds the connection rules — endpoint-policy validation, sync-schedule
//! validation and pairing, config encryption, and the config-replacement guard
//! that a connection's provider is fixed at creation (carrying stored OAuth tokens
//! onto a replacement config) — in one place, factored out of the handler. The
//! service owns the crypto service and the endpoint policy. The external-store
//! actions (verify, picker token) stay in the handler; they load a connection
//! through [`find`](WorkspaceConnectionService::find).

use nvisy_core::net::EndpointPolicy;
use nvisy_postgres::model::{
    NewWorkspaceConnection, NewWorkspaceConnectionSchedule, WorkspaceConnectionSchedule,
};
use nvisy_postgres::query::{
    ConnectionCursor, WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository,
    WorkspaceConnectionSyncRepository,
};
use nvisy_postgres::types::{ConnectionId, CursorPage, CursorPagination};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn, model};
use uuid::Uuid;

use crate::domain::input::{CreateConnectionInput, SyncScheduleInput, UpdateConnectionInput};
use crate::domain::output::FoundConnection;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{ConnectionConfig, CryptoService, StandardCronSchedule, event};

/// Tracing target for connection domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::connection";

/// Creates, reads, updates, and deletes workspace connections.
///
/// Holds the Postgres client (acquiring its own connection per call), the crypto
/// service (to encrypt and decrypt the connection config), and the endpoint policy
/// (to validate custom endpoints at write time). Resolved per request from
/// [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct WorkspaceConnectionService {
    postgres: PgClient,
    crypto: CryptoService,
    endpoint_policy: EndpointPolicy,
}

impl WorkspaceConnectionService {
    /// Creates a [`WorkspaceConnectionService`] over its clients.
    pub fn new(postgres: PgClient, crypto: CryptoService, endpoint_policy: EndpointPolicy) -> Self {
        Self {
            postgres,
            crypto,
            endpoint_policy,
        }
    }

    /// Creates a connection (and, for a schedulable provider given a `sync` block,
    /// its schedule), encrypting the config.
    ///
    /// A disallowed custom endpoint is rejected before store; a `sync` block is
    /// accepted only for a provider that supports scheduled sync. The connection,
    /// its schedule, and the creation event commit together.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        input: CreateConnectionInput,
    ) -> Result<FoundConnection> {
        input
            .config
            .validate_endpoints(self.endpoint_policy)
            .await?;

        if let Some(sync) = &input.sync {
            if !input.config.supports_schedule() {
                return Err(ErrorKind::BadRequest
                    .with_message("This provider does not support scheduled sync"));
            }
            validate_sync_input(sync)?;
        }

        let provider = input.config.provider_id().to_owned();
        let connection_type = input.config.connection_type();
        let encrypted_data = self
            .crypto
            .encrypt_json(origin.workspace_id, &input.config)?;

        let new_connection = NewWorkspaceConnection {
            workspace_id: origin.workspace_id,
            account_id: origin.account_id,
            display_name: input.display_name,
            provider,
            connection_type,
            encrypted_data,
            is_active: input.is_active,
            metadata: None,
        };

        let sync = input.sync;
        let mut conn = self.postgres.get_connection().await?;
        let connection = conn
            .transaction(async |conn| {
                let connection = conn.create_workspace_connection(new_connection).await?;
                if let Some(sync) = sync {
                    conn.create_connection_schedule(NewWorkspaceConnectionSchedule {
                        connection_id: connection.id,
                        sync_mode: Some(sync.sync_mode),
                        schedule_cron: sync.schedule_cron,
                        deletion_policy: Some(sync.deletion_policy),
                    })
                    .await?;
                }
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ConnectionCreated(event::ConnectionCreated {
                        connection_id: connection.id,
                        connection_name: connection.display_name.clone(),
                    }),
                )
                .await?;
                Ok::<_, Error>(connection)
            })
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            connection_id = %ConnectionId::from_uuid(connection.id),
            provider = %connection.provider,
            "Connection created",
        );
        find_connection(
            &mut conn,
            origin.workspace_id,
            ConnectionId::from_uuid(connection.id),
        )
        .await
    }

    /// Lists a workspace's connections, each with its creator, schedule, and last
    /// successful sync time, optionally filtered by provider.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<ConnectionCursor>,
        providers: &[String],
    ) -> Result<CursorPage<FoundConnection>> {
        let mut conn = self.postgres.get_connection().await?;
        let page = conn
            .cursor_list_workspace_connections(workspace_id, pagination, providers)
            .await?;

        // Resolve last-synced and schedules for the whole page in one query each,
        // not per row.
        let ids: Vec<Uuid> = page.items.iter().map(|wc| wc.item.id).collect();
        let mut last_synced_at: std::collections::HashMap<Uuid, jiff::Timestamp> = conn
            .last_successful_sync_at(&ids)
            .await?
            .into_iter()
            .map(|(id, ts)| (id, ts.into()))
            .collect();
        let mut schedules: std::collections::HashMap<Uuid, WorkspaceConnectionSchedule> = conn
            .find_schedules(&ids)
            .await?
            .into_iter()
            .map(|schedule| (schedule.connection_id, schedule))
            .collect();

        Ok(page.map(|connection| {
            let id = connection.item.id;
            FoundConnection {
                schedule: schedules.remove(&id),
                last_synced_at: last_synced_at.remove(&id),
                connection,
            }
        }))
    }

    /// Finds a connection by id with its creator, schedule, and last successful
    /// sync time, or a NotFound.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        connection_id: ConnectionId,
    ) -> Result<FoundConnection> {
        let mut conn = self.postgres.get_connection().await?;
        find_connection(&mut conn, workspace_id, connection_id).await
    }

    /// Updates a connection, returning it with its creator, schedule, and last
    /// successful sync time.
    ///
    /// A replacement config must keep the same provider — changing it would desync
    /// the provider/type columns, schedule, and OAuth tokens — so a differing
    /// provider is rejected. A file-service connection carries its stored OAuth
    /// tokens onto the replacement config, since tokens are never sent by the
    /// client and a refresh may have rotated them. The update, its schedule, and
    /// the event commit together, under a row lock that serializes against a
    /// concurrent token refresh.
    pub async fn update(
        &self,
        origin: event::EventOrigin<'_>,
        connection_id: ConnectionId,
        input: UpdateConnectionInput,
    ) -> Result<FoundConnection> {
        if let Some(config) = &input.config {
            config.validate_endpoints(self.endpoint_policy).await?;
        }

        let mut conn = self.postgres.get_connection().await?;
        let existing = find_connection(&mut conn, origin.workspace_id, connection_id)
            .await?
            .connection
            .item;

        if let Some(sync) = &input.sync {
            if !existing.connection_type.supports_schedule() {
                return Err(ErrorKind::BadRequest
                    .with_message("This provider does not support scheduled sync"));
            }
            validate_sync_input(sync)?;
        }

        let connection_row_id = existing.id;
        let connection_name = input
            .display_name
            .clone()
            .unwrap_or_else(|| existing.display_name.clone());
        let workspace_id = origin.workspace_id;
        let crypto = self.crypto.clone();
        conn.transaction(async move |conn| {
            let Some(current) = conn
                .find_workspace_connection_by_id_for_update(connection_row_id)
                .await?
            else {
                return Err(ErrorKind::NotFound.with_message("WorkspaceConnection not found"));
            };

            let (provider, encrypted_data) = match input.config {
                Some(mut config) => {
                    let stored: ConnectionConfig =
                        crypto.decrypt_json(workspace_id, &current.encrypted_data)?;
                    if config.provider_id() != stored.provider_id() {
                        return Err(ErrorKind::BadRequest.with_message(
                            "A connection's provider cannot be changed; delete and recreate instead",
                        ));
                    }
                    if let (
                        ConnectionConfig::FileService(new),
                        ConnectionConfig::FileService(existing),
                    ) = (&mut config, &stored)
                    {
                        new.set_tokens(existing.tokens().clone());
                    }
                    (
                        Some(config.provider_id().to_owned()),
                        Some(crypto.encrypt_json(workspace_id, &config)?),
                    )
                }
                None => (None, None),
            };

            let update_data = model::UpdateWorkspaceConnection {
                display_name: input.display_name,
                provider,
                is_active: input.is_active,
                encrypted_data,
                ..Default::default()
            };
            conn.update_workspace_connection(connection_row_id, update_data)
                .await?;
            if let Some(sync) = input.sync {
                conn.upsert_connection_schedule(NewWorkspaceConnectionSchedule {
                    connection_id: connection_row_id,
                    sync_mode: Some(sync.sync_mode),
                    schedule_cron: sync.schedule_cron,
                    deletion_policy: Some(sync.deletion_policy),
                })
                .await?;
            }
            conn.emit_event(
                origin,
                event::WorkspaceEvent::ConnectionUpdated(event::ConnectionUpdated {
                    connection_id: connection_row_id,
                    connection_name,
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Connection updated");
        find_connection(&mut conn, origin.workspace_id, connection_id).await
    }

    /// Soft-deletes a connection, recording the event atomically.
    pub async fn delete(
        &self,
        origin: event::EventOrigin<'_>,
        connection_id: ConnectionId,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let existing = find_connection(&mut conn, origin.workspace_id, connection_id)
            .await?
            .connection
            .item;

        conn.transaction(async |conn| {
            conn.delete_workspace_connection(existing.id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::ConnectionDeleted(event::ConnectionDeleted {
                    connection_id: existing.id,
                    connection_name: existing.display_name.clone(),
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Connection deleted");
        Ok(())
    }
}

/// Validates a sync-schedule input: the cron expression, when present, must be
/// valid. Either direction may be scheduled — an import pulls the listing, an
/// export pushes redacted outputs.
fn validate_sync_input(sync: &SyncScheduleInput) -> Result<()> {
    if let Some(cron) = &sync.schedule_cron
        && !StandardCronSchedule.is_valid(cron)
    {
        return Err(ErrorKind::BadRequest.with_message("Invalid cron expression"));
    }
    Ok(())
}

/// Finds a connection within a workspace by id, with its creator, schedule, and
/// last successful sync time, or a NotFound error.
async fn find_connection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_id: ConnectionId,
) -> Result<FoundConnection> {
    let found = conn
        .find_connection_in_workspace_with_creator(workspace_id, connection_id.as_uuid())
        .await?
        .ok_or_else(|| Error::not_found("connection"))?;
    let schedule = conn.find_connection_schedule(found.item.id).await?;
    let last_synced_at = conn
        .last_successful_sync_at(&[found.item.id])
        .await?
        .into_iter()
        .next()
        .map(|(_, ts)| ts.into());
    Ok(FoundConnection {
        connection: found,
        schedule,
        last_synced_at,
    })
}
