//! Application state and dependency injection.

mod account_provisioner;
mod assistant;
mod auth_issuer;
mod avatar;
mod blob_reaper;
mod crypto;
mod detection;
mod engine;
pub mod event;
mod health;
mod infra;
mod integration;
mod notification;
mod oidc;
mod password;
mod policy;
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
use nvisy_postgres::{PgClient, PgConfig};
use nvisy_s3::BlobStore;
pub use nvisy_s3::S3Config;
use nvisy_webhook::WebhookService;
use tokio_util::sync::CancellationToken;

use crate::Result;
use crate::middleware::UploadConfig;
use crate::response::CookieConfig;
pub use crate::service::account_provisioner::AccountProvisioner;
pub use crate::service::assistant::{
    AssistantCoordinator, AssistantJob, AssistantOutboxDrainer, AssistantQueue, AssistantWorker,
};
pub use crate::service::auth_issuer::AuthIssuer;
pub use crate::service::avatar::{AVATAR_CONTENT_TYPE, AvatarService, MAX_AVATAR_UPLOAD_BYTES};
pub use crate::service::blob_reaper::BlobReaper;
pub use crate::service::crypto::{CryptoConfig, CryptoService};
pub(crate) use crate::service::crypto::{CryptoError, HashingReader, LimitedReader, Measurements};
pub(crate) use crate::service::detection::resolve_pinned_policies;
pub use crate::service::detection::{
    DetectionCoordinator, DetectionJob, DetectionOutboxDrainer, DetectionQueue,
    DetectionStatusEvent, DetectionWorker, detection_subject,
};
pub use crate::service::engine::{EngineConfig, EngineService, UnknownFormatToken};
pub use crate::service::health::{HealthCache, HealthConfig};
pub use crate::service::infra::Infra;
pub use crate::service::integration::{
    ConnectionConfig, ConnectionSyncJob, ConnectionSyncService, ConnectionSyncWorker,
    FileConnectorsConfig, FileServiceRedirect, IntegrationConfig, ProviderConfig, SourceEntry,
    StandardCronSchedule, TransferKind, TransferRequest, persist_refreshed_tokens,
};
pub use crate::service::notification::{NotificationEmitter, UnreadCountEvent};
pub use crate::service::oidc::{
    OidcAuthorization, OidcConfig, OidcError, OidcIdentity, OidcService, RedirectKind,
};
pub use crate::service::password::PasswordService;
pub use crate::service::policy::{PolicyService, ResolvedPolicy};
pub use crate::service::run_blob_store::{PurgeOutcome, RunBlobStore};
pub use crate::service::session_keys::{SessionKeys, SessionKeysConfig};
pub use crate::service::user_agent::UserAgentParser;
pub use crate::service::webhook::{WebhookDeliveryWorker, WebhookEmitter};
pub use crate::service::worker::{Worker, WorkerSet};

/// Tracing target for service-state initialization.
const TRACING_TARGET: &str = "nvisy_server::service";

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
    // Shared infrastructure (Postgres, NATS, blob store):
    pub infra: Infra,

    // Encryption service (master key + per-workspace derivation).
    pub crypto: CryptoService,

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

    // In-process wake signal from the detection enqueue path to the outbox
    // drainer, shared by the per-request `DetectionQueue` and the drainer.
    pub detection: DetectionCoordinator,

    // In-process wake signal from the assistant enqueue path to its outbox
    // drainer, shared by the per-request `AssistantQueue` and the drainer.
    pub assistant: AssistantCoordinator,

    // Operational: the app-wide shutdown signal (cancelled once on Ctrl+C/SIGTERM
    // so long-lived handlers and background workers wind down promptly) and the
    // cached health snapshot.
    pub shutdown: CancellationToken,
    pub health_cache: HealthCache,

    // Security services:
    pub password: PasswordService,
    pub session_keys: SessionKeys,
    pub oidc: OidcService,
    pub user_agent_parser: UserAgentParser,

    // Request body size limits (server-wide hard caps):
    pub upload: UploadConfig,

    // Session-cookie policy (the `Secure` attribute) for browser clients.
    pub cookie: CookieConfig,
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
        oidc_config: OidcConfig,
        webhook_service: WebhookService,
        upload_config: UploadConfig,
        cookie_config: CookieConfig,
        s3_config: S3Config,
    ) -> Result<Self> {
        let infra = Infra::from_config(postgres_config, nats_config, s3_config).await?;

        let crypto = CryptoService::from_config(&crypto_config).await?;
        let engine = EngineService::from_config(engine_config).await?;
        let session_keys = SessionKeys::from_config(&session_config).await?;
        let oidc = OidcService::from_config(&oidc_config)?;

        // Session cookies without `Secure` are only safe over plain HTTP on a
        // trusted network (local development or trusted-network self-hosting); a
        // browser will not even store them over HTTPS. Warn loudly so an
        // accidental production misconfiguration is visible.
        if !cookie_config.secure {
            tracing::warn!(
                target: TRACING_TARGET,
                "COOKIE_SECURE is disabled: session cookies are sent without the Secure \
                 attribute. Only use this for local HTTP development or trusted-network \
                 self-hosting, never for an internet-facing deployment.",
            );
        }

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
            crypto.clone(),
            ExternalObjectStore::new(endpoint_policy),
            file_service.clone(),
            integration_config.import_concurrency,
            integration_config.export_concurrency,
        );

        let service_state = Self {
            infra,
            crypto,
            file_service,
            file_service_redirect,
            connection_sync,
            webhook: webhook_service,
            endpoint_policy,
            engine,
            detection: DetectionCoordinator::new(),
            assistant: AssistantCoordinator::new(),
            shutdown: CancellationToken::new(),
            health_cache: HealthCache::new(&health_config, health_checkers),
            password: PasswordService::new(),
            session_keys,
            oidc,
            user_agent_parser: UserAgentParser::new(),
            upload: upload_config,
            cookie: cookie_config,
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
            self.crypto.clone(),
            self.webhook.clone(),
        ));
        workers.spawn(ConnectionSyncWorker::new(
            self.infra.clone(),
            self.crypto.clone(),
            self.connection_sync.clone(),
        ));
        workers.spawn(BlobReaper::new(self.infra.clone(), self.crypto.clone()));
        workers.spawn(event::EventOutboxDrainer::new(self.infra.clone()));
        workers.spawn(DetectionOutboxDrainer::new(
            self.infra.clone(),
            self.detection.clone(),
        ));
        workers.spawn(DetectionWorker::new(
            self.infra.clone(),
            self.engine.clone(),
            RunBlobStore::from_ref(self),
            DetectionQueue::from_ref(self),
        ));
        workers.spawn(AssistantOutboxDrainer::new(
            self.infra.clone(),
            self.assistant.clone(),
        ));
        workers.spawn(AssistantWorker::new(
            self.infra.clone(),
            self.crypto.clone(),
        ));
        workers
    }
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

/// Derives [`FromRef`] for a stateless unit service that holds nothing and
/// operates entirely on the connection passed to each of its methods.
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_unit {
    ($($t:ident),+ $(,)?) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(_state: &ServiceState) -> Self {
                $t
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
    blobs: BlobStore,
);

// Stored fields, in the struct's domain order (infra, crypto, integrations,
// engine, operational, security, limits):
impl_di_field!(
    infra: Infra,
    crypto: CryptoService,
    file_service: FileService,
    file_service_redirect: FileServiceRedirect,
    connection_sync: ConnectionSyncService,
    webhook: WebhookService,
    endpoint_policy: EndpointPolicy,
    engine: EngineService,
    detection: DetectionCoordinator,
    assistant: AssistantCoordinator,
    shutdown: CancellationToken,
    health_cache: HealthCache,
    password: PasswordService,
    session_keys: SessionKeys,
    oidc: OidcService,
    user_agent_parser: UserAgentParser,
    upload: UploadConfig,
    cookie: CookieConfig,
);

// Stateless services, composed from `Infra` on extraction:
impl_di_compose!(
    AvatarService => AvatarService::new,
    WebhookEmitter => WebhookEmitter::new,
    NotificationEmitter => NotificationEmitter::new,
);

// `RunBlobStore` composes from `Infra` and the crypto service (it encrypts and
// decrypts blob content), so it needs a hand-written `FromRef`.
impl axum::extract::FromRef<ServiceState> for RunBlobStore {
    fn from_ref(state: &ServiceState) -> Self {
        RunBlobStore::new(state.infra.clone(), state.crypto.clone())
    }
}

// `DetectionQueue` composes from two singletons — `Infra` and the shared
// `DetectionCoordinator` — so it needs a hand-written `FromRef` rather than the
// compose-from-`Infra`-alone macro above.
impl axum::extract::FromRef<ServiceState> for DetectionQueue {
    fn from_ref(state: &ServiceState) -> Self {
        DetectionQueue::new(state.infra.clone(), state.detection.clone())
    }
}

// `AssistantQueue` likewise composes from `Infra` and the shared
// `AssistantCoordinator`, so it needs a hand-written `FromRef`.
impl axum::extract::FromRef<ServiceState> for AssistantQueue {
    fn from_ref(state: &ServiceState) -> Self {
        AssistantQueue::new(state.infra.clone(), state.assistant.clone())
    }
}

// `ExternalObjectStore` holds only the deployment's endpoint policy:
impl axum::extract::FromRef<ServiceState> for ExternalObjectStore {
    fn from_ref(state: &ServiceState) -> Self {
        ExternalObjectStore::new(state.endpoint_policy)
    }
}

// `AuthIssuer` composes from two security fields — the JWT signing keys and the
// user-agent parser (for session display names):
impl axum::extract::FromRef<ServiceState> for AuthIssuer {
    fn from_ref(state: &ServiceState) -> Self {
        AuthIssuer::new(state.session_keys.clone(), state.user_agent_parser.clone())
    }
}

// Stateless unit services, holding nothing and acting on the connection passed to
// each method:
impl_di_unit!(AccountProvisioner, PolicyService);
