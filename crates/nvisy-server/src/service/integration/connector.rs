//! Turning a stored connection config into a live [`FileSource`].
//!
//! [`Connector`] resolves a typed [`ConnectionConfig`] to the provider family
//! that backs it and, for a cloud file service, refreshes the OAuth token and
//! persists it back so the next transfer starts fresh. Shared by both the import
//! and export paths.

use std::sync::Arc;

use nvisy_file_service::FileService;
use nvisy_file_service::oauth::OAuthTokens;
use nvisy_postgres::model::WorkspaceConnection;

use super::file_source::{FileServiceSource, FileSource, ObjectStoreSource};
use crate::handler::{ErrorKind, Result};
use crate::service::{ConnectionConfig, ExternalObjectStore, Infra, persist_refreshed_tokens};

/// Tracing target for connection sync operations.
const TRACING_TARGET: &str = "nvisy_server::service::sync";

/// Connects a stored connection config to a live file source.
#[derive(Clone)]
pub(super) struct Connector {
    infra: Infra,
    object: ExternalObjectStore,
    cloud: FileService,
}

impl Connector {
    pub(super) fn new(infra: Infra, object: ExternalObjectStore, cloud: FileService) -> Self {
        Self {
            infra,
            object,
            cloud,
        }
    }

    /// Connects to the file source described by a typed connection config,
    /// dispatching to the provider family that backs it. Only transfer-capable
    /// configs resolve; an inference (LLM) connection is rejected.
    ///
    /// For a cloud file service, the OAuth token is refreshed if expired and the
    /// refreshed config is persisted back to `connection` so the next sync starts
    /// from fresh tokens.
    pub(super) async fn file_source(
        &self,
        connection: &WorkspaceConnection,
        config: &ConnectionConfig,
    ) -> Result<Arc<dyn FileSource>> {
        match config {
            ConnectionConfig::ObjectStore(config) => {
                let client = self.object.connect(config).await?;
                Ok(Arc::new(ObjectStoreSource(client)))
            }
            ConnectionConfig::FileService(config) => {
                let connected = self.cloud.connect(config).await?;
                if let Some(refreshed) = connected.refreshed {
                    self.persist_refreshed_tokens(connection, refreshed.tokens().clone())
                        .await?;
                }
                Ok(Arc::new(FileServiceSource(connected.client)))
            }
            ConnectionConfig::Inference(_) => {
                Err(ErrorKind::BadRequest.with_message("Connection does not support file transfer"))
            }
        }
    }

    /// Connects an object-store source, the only family that supports a
    /// whole-listing import. A non-object-store config is rejected: the
    /// listing-based import path is never reached for a file service (it imports
    /// through the picker) or an LLM connection.
    pub(super) async fn object_source(
        &self,
        config: &ConnectionConfig,
    ) -> Result<ObjectStoreSource> {
        match config {
            ConnectionConfig::ObjectStore(config) => {
                Ok(ObjectStoreSource(self.object.connect(config).await?))
            }
            _ => Err(ErrorKind::BadRequest
                .with_message("Whole-listing import is only supported for object stores")),
        }
    }

    /// Persists refreshed OAuth tokens onto the connection's current stored
    /// config (merge-under-read), delegating to the shared helper so a concurrent
    /// config edit is never clobbered.
    async fn persist_refreshed_tokens(
        &self,
        connection: &WorkspaceConnection,
        new_tokens: OAuthTokens,
    ) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        persist_refreshed_tokens(
            &mut conn,
            &self.infra.crypto,
            connection.workspace_id,
            connection.id,
            new_tokens,
        )
        .await?;
        tracing::debug!(
            target: TRACING_TARGET,
            connection_id = %connection.id,
            "Persisted refreshed cloud file OAuth tokens",
        );
        Ok(())
    }
}
