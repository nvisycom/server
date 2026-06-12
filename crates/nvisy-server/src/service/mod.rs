//! Application state and dependency injection.

pub mod crypto;
mod health;
mod security;
mod webhook;

use std::sync::Arc;

use nvisy_core::health::HealthCheck;
use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use nvisy_webhook::WebhookService;

pub use crate::service::crypto::{CryptoConfig, CryptoService};
pub use crate::service::health::{HealthCache, HealthConfig};
pub use crate::service::security::{
    PasswordHasher, PasswordStrength, SessionKeys, SessionKeysConfig, UserAgentParser,
};
pub use crate::service::webhook::{WebhookEmitter, WebhookWorker};
use crate::{Error, Result};

/// Application state.
///
/// Used for the [`State`] extraction (dependency injection).
///
/// [`State`]: axum::extract::State
#[derive(Clone)]
#[must_use = "state does nothing unless you use it"]
pub struct ServiceState {
    // External services:
    pub postgres: PgClient,
    pub nats: NatsClient,
    pub webhook: WebhookService,

    // Security services:
    pub crypto: CryptoService,

    // Internal services:
    pub health_cache: HealthCache,
    pub password_hasher: PasswordHasher,
    pub password_strength: PasswordStrength,
    pub session_keys: SessionKeys,
    pub user_agent_parser: UserAgentParser,
    pub webhook_emitter: WebhookEmitter,
}

impl ServiceState {
    /// Initializes application state from configuration.
    ///
    /// Connects to all external services and loads required resources.
    pub async fn from_config(
        postgres_config: PgConfig,
        nats_config: NatsConfig,
        session_config: SessionKeysConfig,
        crypto_config: CryptoConfig,
        health_config: HealthConfig,
        webhook_service: WebhookService,
    ) -> Result<Self> {
        let postgres_client = connect_postgres(postgres_config).await?;
        let nats_client = connect_nats(nats_config).await?;

        let crypto = CryptoService::from_config(&crypto_config).await?;
        let session_keys = SessionKeys::from_config(&session_config).await?;
        let webhook_emitter = WebhookEmitter::new(postgres_client.clone(), nats_client.clone());

        let health_checkers: Vec<Arc<dyn HealthCheck>> = vec![
            Arc::new(postgres_client.clone()),
            Arc::new(nats_client.clone()),
            Arc::new(webhook_service.clone()),
        ];

        let service_state = Self {
            postgres: postgres_client,
            nats: nats_client,
            webhook: webhook_service,

            crypto,

            health_cache: HealthCache::new(&health_config, health_checkers),
            password_hasher: PasswordHasher::new(),
            password_strength: PasswordStrength::new(),
            session_keys,
            user_agent_parser: UserAgentParser::new(),
            webhook_emitter,
        };

        Ok(service_state)
    }
}

/// Connects to Postgres and applies pending migrations.
async fn connect_postgres(config: PgConfig) -> Result<PgClient> {
    let pg_client = PgClient::new(config).map_err(|e| {
        Error::external("postgres", "Failed to create database client").with_source(e)
    })?;

    pg_client.run_pending_migrations().await.map_err(|e| {
        Error::external("postgres", "Failed to apply database migrations").with_source(e)
    })?;

    Ok(pg_client)
}

/// Connects to the NATS server.
async fn connect_nats(config: NatsConfig) -> Result<NatsClient> {
    NatsClient::connect(config)
        .await
        .map_err(|e| Error::external("NATS", "Failed to connect to NATS").with_source(e))
}

macro_rules! impl_di {
    ($($f:ident: $t:ty),+) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                state.$f.clone()
            }
        }
    )+};
}

// External services:
impl_di!(
    postgres: PgClient,
    nats: NatsClient,
    webhook: WebhookService
);

// Internal services:
impl_di!(
    crypto: CryptoService,
    health_cache: HealthCache,
    password_hasher: PasswordHasher,
    password_strength: PasswordStrength,
    session_keys: SessionKeys,
    user_agent_parser: UserAgentParser,
    webhook_emitter: WebhookEmitter
);
