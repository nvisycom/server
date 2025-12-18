//! Application state and dependency injection.

mod cache;
mod config;
mod security;

use nvisy_core::ocr::OcrService;
use nvisy_core::vlm::VlmService;
use nvisy_nats::NatsClient;
use nvisy_postgres::PgClient;

pub use crate::service::cache::HealthCache;
pub use crate::service::config::ServiceConfig;
pub use crate::service::security::{
    AuthKeysConfig, PasswordHasher, PasswordStrength, RateLimitKey, RateLimiter, SessionKeys,
};
// Re-export error types from crate root for convenience
pub use crate::{Result, Error};

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
    pub ocr_client: OcrService,
    pub vlm_client: VlmService,

    // Internal services:
    pub auth_hasher: PasswordHasher,
    pub password_strength: PasswordStrength,
    pub auth_keys: SessionKeys,
    pub health_cache: HealthCache,
}

impl ServiceState {
    /// Initializes application state from configuration.
    ///
    /// Connects to all external services and loads required resources.
    pub async fn from_config(
        service_config: ServiceConfig,
        ocr_service: OcrService,
        vlm_service: VlmService,
    ) -> Result<Self> {
        let service_state = Self {
            pg_client: service_config.connect_postgres().await?,
            nats_client: service_config.connect_nats().await?,
            ocr_client: ocr_service,
            vlm_client: vlm_service,

            auth_hasher: PasswordHasher::new(),
            password_strength: PasswordStrength::new(),
            auth_keys: service_config.load_auth_keys().await?,
            health_cache: HealthCache::new(),
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
impl_di!(ocr_client: OcrService);
impl_di!(vlm_client: VlmService);

// Internal services:
impl_di!(auth_hasher: PasswordHasher);
impl_di!(password_strength: PasswordStrength);
impl_di!(auth_keys: SessionKeys);
impl_di!(health_cache: HealthCache);
