//! Application state and dependency injection.

use nvisy_nats::NatsClient;
use nvisy_openrouter::LlmClient;
use nvisy_postgres::PgClient;

use crate::service::{
    AuthHasher, AuthKeys, DataCollectionPolicy, HealthCache, PasswordStrength, Result,
    ServiceConfig,
};

/// Application state.
///
/// Used for the [`State`] extraction (dependency injection).
///
/// [`State`]: axum::extract::State
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    pg_client: PgClient,
    llm_client: LlmClient,
    nats_client: NatsClient,

    auth_hasher: AuthHasher,
    password_strength: PasswordStrength,
    regional_policy: DataCollectionPolicy,
    auth_keys: AuthKeys,
    health_cache: HealthCache,
}

impl ServiceState {
    /// Initializes application state from configuration.
    ///
    /// Connects to all external services and loads required resources.
    pub async fn from_config(config: &ServiceConfig) -> Result<Self> {
        let service_state = Self {
            pg_client: config.connect_postgres().await?,
            llm_client: config.connect_llm().await?,
            nats_client: config.connect_nats().await?,

            auth_hasher: config.create_password_hasher()?,
            password_strength: PasswordStrength::new(),
            regional_policy: config.regional_policy(),
            auth_keys: config.load_auth_keys().await?,
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

impl_di!(pg_client: PgClient);
impl_di!(llm_client: LlmClient);
impl_di!(nats_client: NatsClient);

impl_di!(auth_hasher: AuthHasher);
impl_di!(password_strength: PasswordStrength);
impl_di!(regional_policy: DataCollectionPolicy);
impl_di!(auth_keys: AuthKeys);
impl_di!(health_cache: HealthCache);
