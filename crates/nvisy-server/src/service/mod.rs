//! Application state and dependency injection.

mod cache;
mod config;
pub mod crypto;
mod security;
mod webhook;

use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
use nvisy_webhook::WebhookService;

use crate::Result;
pub use crate::service::cache::HealthCache;
pub use crate::service::config::ServiceConfig;
pub use crate::service::security::{
    MasterKey, MasterKeyConfig, PasswordHasher, PasswordStrength, SessionKeys, SessionKeysConfig,
    UserAgentParser,
};
pub use crate::service::webhook::WebhookEmitter;

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
        service_config: ServiceConfig,
        webhook_service: WebhookService,
    ) -> Result<Self> {
        let postgres = service_config.connect_postgres().await?;
        let nats = service_config.connect_nats().await?;

        let master_key = service_config.load_master_key().await?;
        let webhook_emitter = WebhookEmitter::new(postgres.clone(), nats.clone());

        let service_state = Self {
            postgres,
            nats,
            webhook: webhook_service,

            master_key,

            health_cache: HealthCache::new(),
            password_hasher: PasswordHasher::new(),
            password_strength: PasswordStrength::new(),
            session_keys: service_config.load_session_keys().await?,
            user_agent_parser: UserAgentParser::new(),
            webhook_emitter,
        };

        Ok(service_state)
    }
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
