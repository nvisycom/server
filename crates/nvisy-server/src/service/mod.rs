//! Application state and dependency injection.

mod avatar;
mod connection_config;
mod crypto;
mod detection;
mod engine;
mod external_object_store;
mod file_retention;
mod health;
mod infra;
mod notification;
mod password;
mod run_blob_store;
mod session_keys;
mod sync;
mod user_agent;
mod webhook;
mod worker;

use std::sync::Arc;

use nvisy_core::health::HealthCheck;
use nvisy_nats::{NatsClient, NatsConfig};
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use nvisy_webhook::WebhookService;
use tokio_util::sync::CancellationToken;

pub use crate::service::avatar::{AVATAR_CONTENT_TYPE, AvatarService, MAX_AVATAR_UPLOAD_BYTES};
pub use crate::service::connection_config::ConnectionConfig;
pub use crate::service::crypto::{CryptoConfig, CryptoService};
pub(crate) use crate::service::crypto::{CryptoError, HashingReader, Measurements};
pub use crate::service::detection::{
    DetectionJob, DetectionQueue, DetectionWorker, RunStatusEvent, run_subject,
};
pub(crate) use crate::service::detection::{fail_run, resolve_policies};
pub use crate::service::engine::{EngineConfig, EngineService, UnknownFormatToken};
pub use crate::service::external_object_store::ExternalObjectStore;
pub use crate::service::file_retention::FileRetentionWorker;
pub use crate::service::health::{HealthCache, HealthConfig};
pub use crate::service::infra::Infra;
pub use crate::service::notification::{NotificationEmitter, UnreadCountEvent};
pub use crate::service::password::PasswordService;
pub use crate::service::run_blob_store::RunBlobStore;
pub use crate::service::session_keys::{SessionKeys, SessionKeysConfig};
pub use crate::service::sync::{
    ConnectionSyncJob, ConnectionSyncService, ConnectionSyncWorker, DEFAULT_IMPORT_CONCURRENCY,
    StandardCronSchedule, SyncConfig,
};
pub use crate::service::user_agent::UserAgentParser;
pub use crate::service::webhook::{WebhookDeliveryWorker, WebhookEmitter};
pub use crate::service::worker::{Worker, WorkerSet};
use crate::{Error, Result};

/// Application state.
///
/// Used for the [`State`] extraction (dependency injection).
///
/// Only the services that carry live state (or wrap a config-loaded resource)
/// are stored here; the stateless ones — [`AvatarService`], [`RunBlobStore`],
/// [`DetectionQueue`], [`ExternalObjectStore`], [`WebhookEmitter`],
/// [`NotificationEmitter`] — are not fields, but composed on demand from
/// [`Infra`] in their [`FromRef`] impls (a pure move over `Arc`-backed handles).
/// The two stateful singletons ([`ConnectionSyncService`]'s cancellation
/// registry and [`HealthCache`]'s cached snapshot) must be shared, so they are
/// stored.
///
/// [`State`]: axum::extract::State
/// [`FromRef`]: axum::extract::FromRef
#[derive(Clone)]
#[must_use = "state does nothing unless you use it"]
pub struct ServiceState {
    // Shared infrastructure (Postgres, NATS, crypto):
    pub infra: Infra,

    // App-wide shutdown signal: cancelled once on Ctrl+C/SIGTERM so long-lived
    // handlers (SSE streams) and background workers can wind down promptly.
    pub shutdown: CancellationToken,

    // External services:
    pub webhook: WebhookService,

    // Redaction engine:
    pub engine: EngineService,

    // Stateful singletons:
    pub connection_sync: ConnectionSyncService,
    pub health_cache: HealthCache,

    // Security services:
    pub password: PasswordService,
    pub session_keys: SessionKeys,
    pub user_agent_parser: UserAgentParser,
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
        engine_config: EngineConfig,
        health_config: HealthConfig,
        sync_config: SyncConfig,
        webhook_service: WebhookService,
    ) -> Result<Self> {
        let postgres_client = connect_postgres(postgres_config).await?;
        let nats_client = connect_nats(nats_config).await?;

        let crypto = CryptoService::from_config(&crypto_config).await?;
        let infra = Infra::new(postgres_client, nats_client, crypto);

        let engine = EngineService::from_config(engine_config).await?;
        let session_keys = SessionKeys::from_config(&session_config).await?;

        let health_checkers: Vec<Arc<dyn HealthCheck>> = vec![
            Arc::new(infra.postgres.clone()),
            Arc::new(infra.nats.clone()),
            Arc::new(webhook_service.clone()),
        ];

        // The stateful sync singleton composes the stateless emitters/object
        // service from the same `Infra` their `FromRef` impls use.
        let connection_sync = ConnectionSyncService::new(
            infra.clone(),
            ExternalObjectStore::new(),
            WebhookEmitter::new(infra.clone()),
            NotificationEmitter::new(infra.clone()),
            sync_config,
        );

        let service_state = Self {
            infra,
            shutdown: CancellationToken::new(),
            webhook: webhook_service,
            engine,
            connection_sync,
            health_cache: HealthCache::new(&health_config, health_checkers),
            password: PasswordService::new(),
            session_keys,
            user_agent_parser: UserAgentParser::new(),
        };

        Ok(service_state)
    }

    /// Spawns every background worker under the app-wide [`shutdown`] token, so
    /// cancelling it (on Ctrl+C/SIGTERM) stops the workers alongside the
    /// long-lived handlers.
    ///
    /// The stateless collaborators the detection worker needs are composed
    /// through the same [`FromRef`] wiring handlers use. Call
    /// [`WorkerSet::shutdown`] to stop and join them.
    ///
    /// [`shutdown`]: Self::shutdown
    /// [`FromRef`]: axum::extract::FromRef
    pub fn spawn_workers(&self) -> WorkerSet {
        use axum::extract::FromRef;

        let mut workers = WorkerSet::with_token(self.shutdown.clone());
        workers.spawn(WebhookDeliveryWorker::new(
            self.infra.clone(),
            self.webhook.clone(),
        ));
        workers.spawn(ConnectionSyncWorker::new(
            self.infra.clone(),
            self.connection_sync.clone(),
        ));
        workers.spawn(FileRetentionWorker::new(self.infra.clone()));
        workers.spawn(DetectionWorker::new(
            self.infra.clone(),
            self.engine.clone(),
            RunBlobStore::from_ref(self),
            WebhookEmitter::from_ref(self),
            NotificationEmitter::from_ref(self),
            DetectionQueue::from_ref(self),
        ));
        workers
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

/// Derives [`FromRef`] by cloning a stored [`ServiceState`] field.
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_field {
    ($($f:ident: $t:ty),+ $(,)?) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                state.$f.clone()
            }
        }
    )+};
}

/// Derives [`FromRef`] by composing a stateless service from [`Infra`]. The body
/// is a pure move over `Arc`-backed handles, so per-request construction is free.
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_compose {
    ($($t:ty => $ctor:expr),+ $(,)?) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                let ctor: fn(Infra) -> $t = $ctor;
                ctor(state.infra.clone())
            }
        }
    )+};
}

/// Derives [`FromRef`] for a single ambient client by cloning it out of the
/// shared [`Infra`].
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_infra {
    ($($f:ident: $t:ty),+ $(,)?) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                state.infra.$f.clone()
            }
        }
    )+};
}

// The three ambient clients, resolved from the shared `Infra`:
impl_di_infra!(
    postgres: PgClient,
    nats: NatsClient,
    crypto: CryptoService,
);

// Stored fields (external services + stateful singletons + security):
impl_di_field!(
    infra: Infra,
    shutdown: CancellationToken,
    webhook: WebhookService,
    engine: EngineService,
    connection_sync: ConnectionSyncService,
    health_cache: HealthCache,
    password: PasswordService,
    session_keys: SessionKeys,
    user_agent_parser: UserAgentParser,
);

// Stateless services, composed from `Infra` on extraction:
impl_di_compose!(
    AvatarService => AvatarService::new,
    RunBlobStore => RunBlobStore::new,
    DetectionQueue => DetectionQueue::new,
    WebhookEmitter => WebhookEmitter::new,
    NotificationEmitter => NotificationEmitter::new,
);

// `ExternalObjectStore` holds nothing at all:
impl axum::extract::FromRef<ServiceState> for ExternalObjectStore {
    fn from_ref(_state: &ServiceState) -> Self {
        ExternalObjectStore::new()
    }
}
