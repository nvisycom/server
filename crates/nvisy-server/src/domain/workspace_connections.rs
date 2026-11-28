//! Workspace connection domain logic: create, read, list, update, delete.
//!
//! Holds the connection rules — endpoint-policy validation, sync-schedule
//! validation and pairing, config encryption, and the config-replacement guard
//! that a connection's provider is fixed at creation (carrying stored OAuth tokens
//! onto a replacement config) — in one place, factored out of the handler — plus
//! the actions that reach a connection's backing store: verifying it, minting a
//! browser-picker token, and recording a connection authorized through OAuth.

use nvisy_core::net::EndpointPolicy;
use nvisy_file_service::FileService;
use nvisy_file_service::oauth::OAuthTokens;
use nvisy_file_service::provider::FileServiceConfig;
use nvisy_postgres::model::{NewWorkspaceConnection, WorkspaceConnectionSchedule};
use nvisy_postgres::query::{
    WorkspaceConnectionRepository, WorkspaceConnectionScheduleRepository,
    WorkspaceConnectionSyncRepository,
};
use nvisy_postgres::types::{ConnectionId, CursorPage, CursorPagination, TimeCursor};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn, model};
use uuid::Uuid;

use crate::domain::config::replace_config;
use crate::domain::input::{CreateConnectionInput, UpdateConnectionInput};
use crate::domain::output::{ConnectionCheck, FoundConnection, PickerToken};
use crate::response::{Error, ErrorKind, Resource, Result};
use crate::service::event::EventEmitter;
use crate::service::{
    ConnectionConfig, CryptoService, ExternalObjectStore, event, persist_refreshed_tokens,
};

/// Tracing target for connection domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::connection";

/// Creates, reads, updates, and deletes workspace connections.
///
/// Holds the Postgres client (acquiring its own connection per call), the crypto
/// service (to encrypt and decrypt the connection config), the endpoint policy
/// (to validate custom endpoints), and the cloud file service (to reach and
/// refresh file-service connections). Resolved per request from
/// [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspaceConnectionService {
    postgres: PgClient,
    crypto: CryptoService,
    endpoint_policy: EndpointPolicy,
    files: FileService,
}

impl WorkspaceConnectionService {
    /// Creates a [`WorkspaceConnectionService`] over its clients.
    #[must_use]
    pub fn new(
        postgres: PgClient,
        crypto: CryptoService,
        endpoint_policy: EndpointPolicy,
        files: FileService,
    ) -> Self {
        Self {
            postgres,
            crypto,
            endpoint_policy,
            files,
        }
    }

    /// Creates a connection (and, for a schedulable provider given a `sync` block,
    /// its schedule), encrypting the config.
    ///
    /// A disallowed custom endpoint is rejected before store; a `sync` block is
    /// accepted only for a provider that supports scheduled sync. The connection,
    /// its schedule, and the creation event commit together.
    ///
    /// # Errors
    ///
    /// - `BadRequest` if the config uses a custom endpoint the policy forbids, or
    ///   if a `sync` block is given for a provider that does not support scheduled
    ///   sync or carries an invalid cron expression.
    /// - An encryption error if the config cannot be encrypted.
    /// - A database error if a connection or query fails.
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
            sync.validate(input.config.supports_schedule())?;
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
                    conn.create_connection_schedule(sync.into_model(connection.id))
                        .await?;
                }
                conn.emit_event(
                    origin,
                    event::ConnectionCreated {
                        connection_id: connection.id,
                        connection_name: connection.display_name.clone(),
                    },
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
    ///
    /// # Errors
    ///
    /// - A database error if a connection or query fails.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<TimeCursor>,
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
    /// sync time, or a `NotFound`.
    ///
    /// # Errors
    ///
    /// - `NotFound` if no connection with that id exists in the workspace.
    /// - A database error if a connection or query fails.
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
    ///
    /// # Errors
    ///
    /// - `NotFound` if no connection with that id exists in the workspace.
    /// - `BadRequest` if a replacement config uses a custom endpoint the policy
    ///   forbids, changes the connection's provider, or if a `sync` block is
    ///   given for a provider that does not support scheduled sync or carries an
    ///   invalid cron expression.
    /// - An encryption error if the stored config cannot be decrypted or the
    ///   replacement config cannot be encrypted.
    /// - A database error if a connection or query fails.
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
            sync.validate(existing.connection_type.supports_schedule())?;
        }

        let connection_row_id = existing.id;
        let connection_name = input
            .display_name
            .clone()
            .unwrap_or_else(|| existing.display_name.clone());
        let workspace_id = origin.workspace_id;
        conn.transaction(async |conn| {
            let Some(current) = conn
                .find_workspace_connection_by_id_for_update(connection_row_id)
                .await?
            else {
                return Err(Error::not_found(Resource::Connection));
            };

            let (provider, encrypted_data) = input
                .config
                .map(|config| {
                    replace_config(&self.crypto, workspace_id, &current.encrypted_data, config)
                })
                .transpose()?
                .unzip();

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
                conn.upsert_connection_schedule(sync.into_model(connection_row_id))
                    .await?;
            }
            conn.emit_event(
                origin,
                event::ConnectionUpdated {
                    connection_id: connection_row_id,
                    connection_name,
                },
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Connection updated");
        find_connection(&mut conn, origin.workspace_id, connection_id).await
    }

    /// Soft-deletes a connection, recording the event atomically.
    ///
    /// # Errors
    ///
    /// - `NotFound` if no connection with that id exists in the workspace.
    /// - A database error if a connection or query fails.
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
                event::ConnectionDeleted {
                    connection_id: existing.id,
                    connection_name: existing.display_name.clone(),
                },
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Connection deleted");
        Ok(())
    }

    /// Checks a connection against its backing store: an object store must be
    /// reachable, a file service must accept its credentials (refreshed if
    /// expired, with the refreshed tokens stored).
    ///
    /// The provider I/O runs with no pooled connection held: it reaches external
    /// services with no total timeout, so holding one could exhaust the pool.
    /// A failure is a [`ConnectionCheck::Unreachable`] with a safe reason; the
    /// full error is only logged.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the connection does not exist in the workspace.
    /// - A crypto or database error if the config cannot be read, or refreshed
    ///   tokens cannot be stored.
    pub async fn verify(
        &self,
        workspace_id: Uuid,
        connection_id: ConnectionId,
    ) -> Result<ConnectionCheck> {
        let connection = self
            .find(workspace_id, connection_id)
            .await?
            .connection
            .item;
        let config: ConnectionConfig = self
            .crypto
            .decrypt_json(workspace_id, &connection.encrypted_data)?;

        let checked = match config {
            ConnectionConfig::ObjectStore(config) => {
                match ExternalObjectStore::new(self.endpoint_policy)
                    .connect(&config)
                    .await
                {
                    Ok(client) => client.verify_reachable().await,
                    Err(err) => Err(err),
                }
            }
            ConnectionConfig::FileService(config) => match self.files.connect(&config).await {
                Ok(connected) => {
                    // A refresh during verification produces fresh tokens; store
                    // them so the renewed credentials are not thrown away.
                    if let Some(refreshed) = connected.refreshed {
                        self.store_tokens(workspace_id, connection.id, refreshed.tokens().clone())
                            .await?;
                    }
                    connected.client.verify().await
                }
                Err(err) => Err(err),
            },
        };

        Ok(match checked {
            Ok(()) => {
                tracing::info!(target: TRACING_TARGET, %connection_id, "Connection verified");
                ConnectionCheck::Reachable
            }
            Err(err) => {
                tracing::warn!(target: TRACING_TARGET, %connection_id, error = %err, "Connection unreachable");
                ConnectionCheck::Unreachable {
                    reason: err.kind().reason(),
                }
            }
        })
    }

    /// Mints a short-lived provider access token for a browser file picker,
    /// scoped to `resource` when the picker names one (`OneDrive`). A refreshed
    /// token set is stored; the refresh token never leaves the server.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the connection does not exist in the workspace.
    /// - `BadRequest` if the connection is inactive, is not a file service, or
    ///   its provider's picker uses a client-side app key.
    /// - A provider, crypto, or database error if minting or storing fails.
    pub async fn picker_token(
        &self,
        workspace_id: Uuid,
        connection_id: ConnectionId,
        resource: Option<&str>,
    ) -> Result<PickerToken> {
        let connection = self
            .find(workspace_id, connection_id)
            .await?
            .connection
            .item;
        if !connection.is_active {
            return Err(ErrorKind::BadRequest.with_message("Connection is not active"));
        }

        let config: ConnectionConfig = self
            .crypto
            .decrypt_json(workspace_id, &connection.encrypted_data)?;
        let ConnectionConfig::FileService(config) = config else {
            return Err(ErrorKind::BadRequest
                .with_message("Picker tokens are only available for file services"));
        };
        if !config.provider.picker_needs_user_token() {
            return Err(ErrorKind::BadRequest.with_message(
                "This provider's picker uses a client-side app key, not a server token",
            ));
        }

        let picker = self.files.mint_picker_token(&config, resource).await?;
        if let Some(refreshed) = picker.refreshed {
            self.store_tokens(workspace_id, connection.id, refreshed.tokens().clone())
                .await?;
        }
        Ok(PickerToken {
            access_token: picker.access_token,
            expires_at: picker.expires_at,
        })
    }

    /// Records a file-service connection whose credentials an OAuth flow just
    /// obtained, with its created event, in one transaction. A file service is
    /// request-time only (picker-driven import, per-file export), so it gets no
    /// schedule.
    ///
    /// # Errors
    ///
    /// - A crypto error if the config cannot be encrypted.
    /// - A database error if the insert or the event fails.
    pub async fn create_authorized(
        &self,
        origin: event::EventOrigin<'_>,
        display_name: String,
        config: FileServiceConfig,
    ) -> Result<Uuid> {
        let config = ConnectionConfig::FileService(config);
        let new_connection = NewWorkspaceConnection {
            workspace_id: origin.workspace_id,
            account_id: origin.account_id,
            display_name,
            provider: config.provider_id().to_owned(),
            connection_type: config.connection_type(),
            encrypted_data: self.crypto.encrypt_json(origin.workspace_id, &config)?,
            is_active: Some(true),
            metadata: None,
        };

        let mut conn = self.postgres.get_connection().await?;
        conn.transaction(async |conn| {
            let connection = conn.create_workspace_connection(new_connection).await?;
            conn.emit_event(
                origin,
                event::ConnectionCreated {
                    connection_id: connection.id,
                    connection_name: connection.display_name.clone(),
                },
            )
            .await?;
            Ok::<_, Error>(connection.id)
        })
        .await
    }

    /// Stores a connection's refreshed OAuth tokens.
    async fn store_tokens(
        &self,
        workspace_id: Uuid,
        connection_id: Uuid,
        tokens: OAuthTokens,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        persist_refreshed_tokens(&mut conn, &self.crypto, workspace_id, connection_id, tokens).await
    }
}

/// Finds a connection within a workspace by id, with its creator, schedule, and
/// last successful sync time, or a `NotFound` error.
async fn find_connection(
    conn: &mut PgConn,
    workspace_id: Uuid,
    connection_id: ConnectionId,
) -> Result<FoundConnection> {
    let found = conn
        .find_connection_in_workspace_with_creator(workspace_id, connection_id.as_uuid())
        .await?
        .ok_or_else(|| Error::not_found(Resource::Connection))?;
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
