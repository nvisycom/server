//! Application state and dependency injection.

mod avatar;
mod chat;
mod crypto;
mod detection;
mod engine;
mod event;
mod file_reaper;
mod health;
mod infra;
mod integration;
mod notification;
mod password;
mod run_blob_store;
mod session_keys;
mod user_agent;
mod webhook;
mod worker;

use std::sync::Arc;

use nvisy_core::health::HealthCheck;
use nvisy_core::net::EndpointPolicy;
use nvisy_file_service::FileService;
use nvisy_nats::{NatsClient, NatsConfig};
pub use nvisy_object_store::client::ExternalObjectStore;
use nvisy_postgres::{PgClient, PgClientMigrationExt, PgConfig};
use nvisy_s3::BlobStore;
pub use nvisy_s3::S3Config;
use nvisy_webhook::WebhookService;
use tokio_util::sync::CancellationToken;

use crate::middleware::UploadConfig;
pub use crate::service::avatar::{AVATAR_CONTENT_TYPE, AvatarService, MAX_AVATAR_UPLOAD_BYTES};
pub use crate::service::chat::{ChatService, TurnLocation};
pub use crate::service::crypto::{CryptoConfig, CryptoService};
pub(crate) use crate::service::crypto::{CryptoError, HashingReader, LimitedReader, Measurements};
pub(crate) use crate::service::detection::resolve_policies;
pub use crate::service::detection::{
    DetectionJob, DetectionOutboxDrainer, DetectionQueue, DetectionStatusEvent, DetectionWorker,
    detection_subject,
};
pub use crate::service::engine::{EngineConfig, EngineService, UnknownFormatToken};
pub use crate::service::event::{
    ConnectionRef, DetectionRef, EventEmitter, EventOrigin, EventOutboxDrainer, FileRef, InviteRef,
    MemberRef, PipelineRef, PolicyRef, WebhookRef, WorkspaceEvent, WorkspaceRef, event_outbox_row,
};
pub use crate::service::file_reaper::FileReaper;
pub use crate::service::health::{HealthCache, HealthConfig};
pub use crate::service::infra::Infra;
pub use crate::service::integration::{
    ConnectionConfig, ConnectionSyncJob, ConnectionSyncService, ConnectionSyncWorker,
    FileConnectorsConfig, FileServiceRedirect, IntegrationConfig, SourceEntry,
    StandardCronSchedule, TransferKind, TransferRequest, persist_refreshed_tokens,
};
pub use crate::service::notification::{NotificationEmitter, UnreadCountEvent};
pub use crate::service::password::PasswordService;
pub use crate::service::run_blob_store::{PurgeOutcome, RunBlobStore};
pub use crate::service::session_keys::{SessionKeys, SessionKeysConfig};
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

    // Integrations: external connectors and the sync engine that drives them.
    pub file_service: FileService,
    /// Frontend URL the cloud file OAuth callback redirects to when done.
    pub file_service_redirect: FileServiceRedirect,
    pub connection_sync: ConnectionSyncService,
    pub webhook: WebhookService,
    /// How caller-supplied connection endpoints are validated (SSRF posture).
    pub endpoint_policy: EndpointPolicy,

    // Redaction engine:
    pub engine: EngineService,

    // Operational: the app-wide shutdown signal (cancelled once on Ctrl+C/SIGTERM
    // so long-lived handlers and background workers wind down promptly) and the
    // cached health snapshot.
    pub shutdown: CancellationToken,
    pub health_cache: HealthCache,

    // Security services:
    pub password: PasswordService,
    pub session_keys: SessionKeys,
    pub user_agent_parser: UserAgentParser,

    // Request body size limits (server-wide hard caps):
    pub upload: UploadConfig,
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
        integration_config: IntegrationConfig,
        file_connectors_config: FileConnectorsConfig,
        webhook_service: WebhookService,
        upload_config: UploadConfig,
        s3_config: S3Config,
    ) -> Result<Self> {
        let postgres_client = connect_postgres(postgres_config).await?;
        let nats_client = connect_nats(nats_config).await?;
        let blobs = connect_blobs(s3_config).await?;

        let crypto = CryptoService::from_config(&crypto_config).await?;
        let infra = Infra::new(postgres_client, nats_client, crypto, blobs);

        let engine = EngineService::from_config(engine_config).await?;
        let session_keys = SessionKeys::from_config(&session_config).await?;

        let health_checkers: Vec<Arc<dyn HealthCheck>> = vec![
            Arc::new(infra.postgres.clone()),
            Arc::new(infra.nats.clone()),
            Arc::new(infra.blobs.clone()),
            Arc::new(webhook_service.clone()),
        ];

        // The stateful sync singleton composes the stateless emitters/object
        // service from the same `Infra` their `FromRef` impls use. The cloud
        // file service (HTTP client + OAuth apps) is built by the crate; the
        // post-auth redirect is a host-side concern kept alongside it.
        let (file_service, file_service_redirect) = file_connectors_config.build()?;
        let endpoint_policy = integration_config.endpoint_policy;
        let connection_sync = ConnectionSyncService::new(
            infra.clone(),
            ExternalObjectStore::new(endpoint_policy),
            file_service.clone(),
            integration_config.import_concurrency,
        );

        let service_state = Self {
            infra,
            file_service,
            file_service_redirect,
            connection_sync,
            webhook: webhook_service,
            endpoint_policy,
            engine,
            shutdown: CancellationToken::new(),
            health_cache: HealthCache::new(&health_config, health_checkers),
            password: PasswordService::new(),
            session_keys,
            user_agent_parser: UserAgentParser::new(),
            upload: upload_config,
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
        workers.spawn(FileReaper::new(self.infra.clone()));
        workers.spawn(EventOutboxDrainer::new(self.infra.clone()));
        workers.spawn(DetectionOutboxDrainer::new(self.infra.clone()));
        workers.spawn(DetectionWorker::new(
            self.infra.clone(),
            self.engine.clone(),
            RunBlobStore::from_ref(self),
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

/// Connects to the S3-compatible blob store and verifies it is reachable.
///
/// The AWS SDK builds its client lazily, so [`BlobStore::connect`] alone never
/// touches the network. A follow-up [`ping`](BlobStore::ping) makes a bad
/// endpoint, wrong credentials, or missing bucket fail at startup — matching the
/// fail-fast contract of the Postgres and NATS connectors — rather than at the
/// first upload.
async fn connect_blobs(config: S3Config) -> Result<BlobStore> {
    let blobs = BlobStore::connect(&config)
        .await
        .map_err(|e| Error::external("S3", "Failed to connect to the blob store").with_source(e))?;

    blobs.ping().await.map_err(|e| {
        Error::external("S3", "Blob store is unreachable or its bucket is missing").with_source(e)
    })?;

    Ok(blobs)
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

// The ambient clients, resolved from the shared `Infra`:
impl_di_infra!(
    postgres: PgClient,
    nats: NatsClient,
    crypto: CryptoService,
    blobs: BlobStore,
);

// Stored fields, in the struct's domain order (infra, integrations, engine,
// operational, security, limits):
impl_di_field!(
    infra: Infra,
    file_service: FileService,
    file_service_redirect: FileServiceRedirect,
    connection_sync: ConnectionSyncService,
    webhook: WebhookService,
    endpoint_policy: EndpointPolicy,
    engine: EngineService,
    shutdown: CancellationToken,
    health_cache: HealthCache,
    password: PasswordService,
    session_keys: SessionKeys,
    user_agent_parser: UserAgentParser,
    upload: UploadConfig,
);

// Stateless services, composed from `Infra` on extraction:
impl_di_compose!(
    AvatarService => AvatarService::new,
    ChatService => ChatService::new,
    RunBlobStore => RunBlobStore::new,
    DetectionQueue => DetectionQueue::new,
    WebhookEmitter => WebhookEmitter::new,
    NotificationEmitter => NotificationEmitter::new,
);

// `ExternalObjectStore` holds only the deployment's endpoint policy:
impl axum::extract::FromRef<ServiceState> for ExternalObjectStore {
    fn from_ref(state: &ServiceState) -> Self {
        ExternalObjectStore::new(state.endpoint_policy)
    }
}
