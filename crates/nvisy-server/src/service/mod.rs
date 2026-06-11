//! Application state and dependency injection.

mod cache;
pub mod crypto;
mod security;
mod webhook;

use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use nvisy_webhook::WebhookService;

pub use crate::service::cache::HealthCache;
pub use crate::service::security::{
    MasterKey, MasterKeyConfig, PasswordHasher, PasswordStrength, SessionKeys, SessionKeysConfig,
    UserAgentParser,
};
pub use crate::service::webhook::WebhookEmitter;
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
    pub master_key: MasterKey,

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
        master_key_config: MasterKeyConfig,
        webhook_service: WebhookService,
    ) -> Result<Self> {
        let postgres = connect_postgres(postgres_config).await?;
        let nats = connect_nats(nats_config).await?;

        let master_key = MasterKey::from_config(&master_key_config).await?;
        let session_keys = SessionKeys::from_config(&session_config).await?;
        let webhook_emitter = WebhookEmitter::new(postgres.clone(), nats.clone());

        let service_state = Self {
            postgres,
            nats,
            webhook: webhook_service,

            master_key,

            health_cache: HealthCache::new(),
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
impl_di!(postgres: PgClient);
impl_di!(nats: NatsClient);
impl_di!(webhook: WebhookService);

// Security services:
impl_di!(master_key: MasterKey);

// Internal services:
impl_di!(health_cache: HealthCache);
impl_di!(password_hasher: PasswordHasher);
impl_di!(password_strength: PasswordStrength);
impl_di!(session_keys: SessionKeys);
impl_di!(user_agent_parser: UserAgentParser);
impl_di!(webhook_emitter: WebhookEmitter);
