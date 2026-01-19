//! Application state and dependency injection.

mod cache;
mod config;
mod integration;
mod security;

use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;
use nvisy_rig::RigService;
use nvisy_runtime::RuntimeService;
use nvisy_webhook::WebhookService;

// Re-export archive types for handler use
pub use nvisy_runtime::{ArchiveFormat, ArchiveResult, ArchiveService};

use crate::Result;
pub use crate::service::cache::HealthCache;
pub use crate::service::config::ServiceConfig;
pub use crate::service::integration::IntegrationProvider;
pub use crate::service::security::{
    PasswordHasher, PasswordStrength, SessionKeys, SessionKeysConfig, UserAgentParser,
};

/// Application state.
///
/// Used for the [`State`] extraction (dependency injection).
///
/// [`State`]: axum::extract::State
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    // External services:
    pub postgres: PgClient,
    pub nats: NatsClient,
    pub webhook: WebhookService,

    // AI & document services:
    pub rig: RigService,
    pub runtime: RuntimeService,
    pub archive: ArchiveService,

    // Internal services:
    pub health_cache: HealthCache,
    pub integration_provider: IntegrationProvider,
    pub password_hasher: PasswordHasher,
    pub password_strength: PasswordStrength,
    pub session_keys: SessionKeys,
    pub user_agent_parser: UserAgentParser,
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

        // Initialize AI services
        let rig = RigService::new(
            service_config.rig_config.clone(),
            postgres.clone(),
            nats.clone(),
        )
        .await
        .map_err(|e| {
            crate::Error::internal("rig", "Failed to initialize rig service").with_source(e)
        })?;

        let service_state = Self {
            postgres,
            nats,
            webhook: webhook_service,

            rig,
            runtime: RuntimeService::new(),
            archive: ArchiveService::new(),

            health_cache: HealthCache::new(),
            integration_provider: IntegrationProvider::new(),
            password_hasher: PasswordHasher::new(),
            password_strength: PasswordStrength::new(),
            session_keys: service_config.load_session_keys().await?,
            user_agent_parser: UserAgentParser::new(),
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

// AI and document services:
impl_di!(rig: RigService);
impl_di!(runtime: RuntimeService);
impl_di!(archive: ArchiveService);

// Internal services:
impl_di!(health_cache: HealthCache);
impl_di!(integration_provider: IntegrationProvider);
impl_di!(password_hasher: PasswordHasher);
impl_di!(password_strength: PasswordStrength);
impl_di!(session_keys: SessionKeys);
impl_di!(user_agent_parser: UserAgentParser);
