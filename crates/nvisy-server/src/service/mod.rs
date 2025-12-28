//! Application state and dependency injection.

mod cache;
mod compression;
mod config;
mod security;

use nvisy_core::AiServices;
use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;

pub use crate::service::cache::HealthCache;
pub use crate::service::compression::{ArchiveFormat, ArchiveService};
pub use crate::service::config::ServiceConfig;
pub use crate::service::security::{AuthConfig, AuthKeys, PasswordHasher, PasswordStrength};
// Re-export error types from crate root for convenience
pub use crate::{Error, Result};

/// Application state.
///
/// Used for the [`State`] extraction (dependency injection).
///
/// [`State`]: axum::extract::State
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    // External services:
    pub pg_client: PgClient,
    pub nats_client: NatsClient,
    pub ai_services: AiServices,

    // Internal services:
    pub auth_hasher: PasswordHasher,
    pub password_strength: PasswordStrength,
    pub auth_keys: AuthKeys,
    pub health_cache: HealthCache,
    pub archive: ArchiveService,
}

impl ServiceState {
    /// Initializes application state from configuration.
    ///
    /// Connects to all external services and loads required resources.
    pub async fn from_config(
        service_config: ServiceConfig,
        ai_services: AiServices,
    ) -> Result<Self> {
        let service_state = Self {
            pg_client: service_config.connect_postgres().await?,
            nats_client: service_config.connect_nats().await?,
            ai_services,

            auth_hasher: PasswordHasher::new(),
            password_strength: PasswordStrength::new(),
            auth_keys: service_config.load_auth_keys().await?,
            health_cache: HealthCache::new(),
            archive: ArchiveService::new(),
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
impl_di!(pg_client: PgClient);
impl_di!(nats_client: NatsClient);
impl_di!(ai_services: AiServices);

// Internal services:
impl_di!(auth_hasher: PasswordHasher);
impl_di!(password_strength: PasswordStrength);
impl_di!(auth_keys: AuthKeys);
impl_di!(health_cache: HealthCache);
impl_di!(archive: ArchiveService);
