//! Workspace inference-provider domain logic: create, read, list, update, delete.
//!
//! Holds the provider rules — endpoint-policy validation, config encryption, and
//! the config-replacement guard that a provider's provider is fixed at creation —
//! in one place, factored out of the handler. The service owns the crypto service
//! (a create encrypts the config, an update decrypts to guard the provider) and
//! the endpoint policy (to reject a disallowed custom endpoint before store).

use nvisy_core::net::EndpointPolicy;
use nvisy_postgres::model::{NewWorkspaceProvider, WorkspaceProvider};
use nvisy_postgres::query::{ProviderCursor, WorkspaceProviderRepository};
use nvisy_postgres::types::{CursorPage, CursorPagination, ProviderId, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn, model};
use uuid::Uuid;

use crate::domain::input::{CreateProviderInput, UpdateProviderInput};
use crate::response::{Error, ErrorKind, Result};
use crate::service::event::EventEmitter;
use crate::service::{CryptoService, ProviderConfig, event};

/// Tracing target for provider domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::provider";

/// Creates, reads, updates, and deletes workspace inference providers.
///
/// Holds the Postgres client (acquiring its own connection per call), the crypto
/// service (to encrypt and decrypt the provider config), and the endpoint policy
/// (to validate custom endpoints at write time). Resolved per request from
/// [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspaceProviderService {
    postgres: PgClient,
    crypto: CryptoService,
    endpoint_policy: EndpointPolicy,
}

impl WorkspaceProviderService {
    /// Creates a [`WorkspaceProviderService`] over its clients.
    #[must_use]
    pub fn new(postgres: PgClient, crypto: CryptoService, endpoint_policy: EndpointPolicy) -> Self {
        Self {
            postgres,
            crypto,
            endpoint_policy,
        }
    }

    /// Creates a provider, encrypting its config.
    ///
    /// The provider and its model type are derived from the typed config so they
    /// can never disagree with it; the full config is encrypted at rest. A
    /// disallowed custom endpoint is rejected before the config is stored. The
    /// provider and its creation event commit together.
    ///
    /// # Errors
    /// - `BadRequest` if the config names a custom endpoint the endpoint policy
    ///   disallows.
    /// - A crypto error if encrypting the config fails.
    /// - A database error if the query fails.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        input: CreateProviderInput,
    ) -> Result<WithAccountRef<WorkspaceProvider>> {
        input
            .config
            .validate_endpoints(self.endpoint_policy)
            .await?;

        let provider = input.config.provider_id().to_owned();
        let provider_type = input.config.provider_type();
        let encrypted_data = self
            .crypto
            .encrypt_json(origin.workspace_id, &input.config)?;

        let new_provider = NewWorkspaceProvider {
            workspace_id: origin.workspace_id,
            account_id: origin.account_id,
            display_name: input.display_name,
            provider,
            provider_type,
            encrypted_data,
            is_active: input.is_active,
            metadata: None,
        };

        let mut conn = self.postgres.get_connection().await?;
        let created = conn
            .transaction(async |conn| {
                let created = conn.create_workspace_provider(new_provider).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::ProviderCreated(event::ProviderCreated {
                        provider_id: created.id,
                        provider_name: created.display_name.clone(),
                    }),
                )
                .await?;
                Ok::<_, Error>(created)
            })
            .await?;

        tracing::info!(
            target: TRACING_TARGET,
            provider_id = %ProviderId::from_uuid(created.id),
            provider = %created.provider,
            "Provider created",
        );
        find_provider(&mut conn, origin.workspace_id, created.id).await
    }

    /// Lists a workspace's providers, each with its creator, optionally filtered by
    /// provider.
    ///
    /// # Errors
    /// A database error if the query fails.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<ProviderCursor>,
        providers: &[String],
    ) -> Result<CursorPage<WithAccountRef<WorkspaceProvider>>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_providers(workspace_id, pagination, providers)
            .await?)
    }

    /// Finds a provider by id with its creator, or a `NotFound`.
    ///
    /// # Errors
    /// - `NotFound` if the provider does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        provider_id: ProviderId,
    ) -> Result<WithAccountRef<WorkspaceProvider>> {
        let mut conn = self.postgres.get_connection().await?;
        find_provider(&mut conn, workspace_id, provider_id.as_uuid()).await
    }

    /// Updates a provider, returning it with its creator.
    ///
    /// A replacement config must keep the same provider — changing it would desync
    /// the `provider/provider_type` columns — so a differing provider is rejected
    /// rather than silently migrated. A disallowed custom endpoint is rejected
    /// before store. The update and its event commit together.
    ///
    /// # Errors
    /// - `NotFound` if the provider does not exist in the workspace.
    /// - `BadRequest` if a replacement config names a disallowed custom endpoint, or
    ///   changes the provider away from the stored one.
    /// - A crypto error if decrypting the stored config or encrypting the
    ///   replacement fails.
    /// - A database error if the query fails.
    pub async fn update(
        &self,
        origin: event::EventOrigin<'_>,
        provider_id: ProviderId,
        input: UpdateProviderInput,
    ) -> Result<WithAccountRef<WorkspaceProvider>> {
        if let Some(config) = &input.config {
            config.validate_endpoints(self.endpoint_policy).await?;
        }

        let mut conn = self.postgres.get_connection().await?;
        let existing = find_provider(&mut conn, origin.workspace_id, provider_id.as_uuid())
            .await?
            .item;

        let provider_row_id = existing.id;
        let provider_name = input
            .display_name
            .clone()
            .unwrap_or_else(|| existing.display_name.clone());
        let workspace_id = origin.workspace_id;
        let crypto = self.crypto.clone();
        conn.transaction(async move |conn| {
            let (provider, encrypted_data) = match input.config {
                Some(config) => {
                    let stored: ProviderConfig =
                        crypto.decrypt_json(workspace_id, &existing.encrypted_data)?;
                    if config.provider_id() != stored.provider_id() {
                        return Err(ErrorKind::BadRequest.with_message(
                            "A provider's provider cannot be changed; delete and recreate instead",
                        ));
                    }
                    (
                        Some(config.provider_id().to_owned()),
                        Some(crypto.encrypt_json(workspace_id, &config)?),
                    )
                }
                None => (None, None),
            };

            let update_data = model::UpdateWorkspaceProvider {
                display_name: input.display_name,
                provider,
                is_active: input.is_active,
                encrypted_data,
                ..Default::default()
            };
            conn.update_workspace_provider(provider_row_id, update_data)
                .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::ProviderUpdated(event::ProviderUpdated {
                    provider_id: provider_row_id,
                    provider_name,
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Provider updated");
        find_provider(&mut conn, origin.workspace_id, provider_id.as_uuid()).await
    }

    /// Soft-deletes a provider, recording the event atomically.
    ///
    /// # Errors
    /// - `NotFound` if the provider does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn delete(
        &self,
        origin: event::EventOrigin<'_>,
        provider_id: ProviderId,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let existing = find_provider(&mut conn, origin.workspace_id, provider_id.as_uuid())
            .await?
            .item;

        conn.transaction(async |conn| {
            conn.delete_workspace_provider(existing.id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::ProviderDeleted(event::ProviderDeleted {
                    provider_id: existing.id,
                    provider_name: existing.display_name.clone(),
                }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Provider deleted");
        Ok(())
    }
}

/// Finds a provider within a workspace by id, with its creator, or a `NotFound`.
async fn find_provider(
    conn: &mut PgConn,
    workspace_id: Uuid,
    provider_id: Uuid,
) -> Result<WithAccountRef<WorkspaceProvider>> {
    conn.find_provider_in_workspace_with_creator(workspace_id, provider_id)
        .await?
        .ok_or_else(|| Error::not_found("provider"))
}
