//! Application state and dependency injection.

use std::collections::HashMap;
use std::sync::Arc;

use nvisy_minio::MinioClient;
use nvisy_postgres::PgDatabase;
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;

use crate::handler::project_websocket::ProjectWsMessage;
use crate::service::auth::{AuthHasher, AuthKeys};
use crate::service::policy::RegionalPolicy;
use crate::service::{PasswordStrength, Result, ServiceConfig};

/// Shared state for project WebSocket broadcast channels.
///
/// Each project has its own broadcast channel for real-time communication.
pub type ProjectChannels = Arc<RwLock<HashMap<Uuid, broadcast::Sender<ProjectWsMessage>>>>;

/// Application state.
///
/// Used for the [`State`] extraction (dependency injection).
///
/// [`State`]: axum::extract::State
#[must_use = "state does nothing unless you use it"]
#[derive(Clone)]
pub struct ServiceState {
    pg_database: PgDatabase,
    minio_client: MinioClient,

    auth_hasher: AuthHasher,
    password_strength: PasswordStrength,
    regional_policy: RegionalPolicy,
    auth_keys: AuthKeys,

    /// WebSocket broadcast channels for projects.
    project_channels: ProjectChannels,
}

impl ServiceState {
    /// Initializes application state from configuration.
    ///
    /// Connects to all external services and loads required resources.
    pub async fn from_config(config: &ServiceConfig) -> Result<Self> {
        let service_state = Self {
            pg_database: config.connect_postgres().await?,
            minio_client: config.connect_file_storage().await?,

            auth_hasher: config.create_password_hasher()?,
            password_strength: PasswordStrength::new(),
            regional_policy: config.regional_policy(),
            auth_keys: config.load_auth_keys().await?,

            project_channels: Arc::new(RwLock::new(HashMap::new())),
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

impl_di!(pg_database: PgDatabase);
impl_di!(minio_client: MinioClient);

impl_di!(auth_hasher: AuthHasher);
impl_di!(password_strength: PasswordStrength);
impl_di!(regional_policy: RegionalPolicy);
impl_di!(auth_keys: AuthKeys);
impl_di!(project_channels: ProjectChannels);
