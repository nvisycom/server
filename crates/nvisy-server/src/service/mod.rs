//! Application state and dependency injection.

mod account_provisioner;
mod auth_flow;
mod auth_issuer;
mod avatar;
mod crypto;
mod emitter;
mod engine;
pub mod event;
mod health;
mod infra;
mod integration;
mod oidc;
mod password;
mod queue;
mod run_blob_store;
mod session_keys;
mod user_agent;

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

use crate::middleware::UploadConfig;
use crate::response::CookieConfig;
pub use crate::service::account_provisioner::AccountProvisioner;
pub use crate::service::auth_flow::SignInService;
pub use crate::service::auth_issuer::AuthIssuer;
pub use crate::service::avatar::{AVATAR_CONTENT_TYPE, AvatarService, MAX_AVATAR_UPLOAD_BYTES};
pub use crate::service::crypto::{CryptoConfig, CryptoService};
pub(crate) use crate::service::crypto::{CryptoError, HashingReader, LimitedReader, Measurements};
pub use crate::service::emitter::{NotificationEmitter, UnreadCountEvent, WebhookEmitter};
pub use crate::service::engine::{EngineConfig, EngineService, UnknownFormatToken};
pub use crate::service::health::{HealthCache, HealthConfig};
pub use crate::service::infra::Infra;
pub use crate::service::integration::{
    ConnectionConfig, ConnectionSyncService, FileConnectorsConfig, FileServiceRedirect,
    IntegrationConfig, ProviderConfig, StandardCronSchedule, TransferKind, TransferRequest,
    persist_refreshed_tokens,
};
pub use crate::service::oidc::{
    CallbackOutcome, ConsumedFlow, OidcAuthorization, OidcConfig, OidcConfigured, OidcError,
    OidcIdentity, OidcPurpose, OidcService, RedirectKind,
};
pub use crate::service::password::PasswordService;
pub use crate::service::queue::{AssistantQueue, DetectionQueue};
pub use crate::service::run_blob_store::{PurgeOutcome, RunBlobStore};
pub use crate::service::session_keys::{SessionKeys, SessionKeysConfig};
pub use crate::service::user_agent::UserAgentParser;
use crate::worker::assistant::{AssistantOutboxDrainer, AssistantWorker};
use crate::worker::detection::{DetectionOutboxDrainer, DetectionWorker};
use crate::worker::event::EventOutboxDrainer;
use crate::worker::integration::ConnectionSyncWorker;
use crate::worker::reaper::BlobReaper;
use crate::worker::webhook::WebhookDeliveryWorker;
use crate::worker::{Coordinator, WorkerSet, ensure_streams};
use crate::{Result, domain};

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
#[derive(Clone, axum::extract::FromRef)]
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
    // drainer, shared by the per-request `DetectionQueue` and the drainer. Not
    // extracted directly (and shares its type with `assistant`), so it is skipped;
    // `DetectionQueue`'s `FromRef` reads it.
    #[from_ref(skip)]
    pub detection: Coordinator,

    // In-process wake signal from the assistant enqueue path to its outbox
    // drainer, shared by the per-request `AssistantQueue` and the drainer. Skipped
    // for the same reason as `detection`.
    #[from_ref(skip)]
    pub assistant: Coordinator,

    // Operational: the app-wide shutdown signal (cancelled once on Ctrl+C/SIGTERM
    // so long-lived handlers and background workers wind down promptly) and the
    // cached health snapshot.
    pub shutdown: CancellationToken,
    pub health_cache: HealthCache,

    // Security services:
    pub password: PasswordService,
    pub session_keys: SessionKeys,
    // Password login/signup/logout orchestration. Holds only cheap `Arc`-backed
    // handles, so it is stored (its `FromRef` is a field clone).
    pub sign_in: SignInService,
    // The startup-loaded OIDC configuration (HTTP client + parsed providers). The
    // per-request `OidcService` is composed from this plus the ambient
    // collaborators in its `FromRef` — the one service that is composed rather than
    // stored, because it pairs an expensive immutable core with per-request handles.
    // Not extracted directly (only `OidcService` is), so it is skipped.
    #[from_ref(skip)]
    pub oidc: OidcConfigured,
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

        // Reconcile every JetStream stream once, up front, so the publishers and
        // subscribers built per use later are cheap handles that assume their
        // stream already exists.
        ensure_streams(&infra.nats).await?;

        let crypto = CryptoService::from_config(&crypto_config).await?;
        let engine = EngineService::from_config(engine_config).await?;
        let session_keys = SessionKeys::from_config(&session_config).await?;
        let oidc = OidcConfigured::from_config(&oidc_config)?;

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

        // The security services and the sign-in orchestration built over them. The
        // auth issuer (JWT keys + user-agent parser) is also the collaborator
        // `SignInService` mints sessions through, so it is assembled here and the
        // two parts are stored separately (each has its own `FromRef`).
        let password = PasswordService::new();
        let user_agent_parser = UserAgentParser::new();
        let issuer = AuthIssuer::new(session_keys.clone(), user_agent_parser.clone());
        let sign_in = SignInService::new(infra.postgres.clone(), password.clone(), issuer);

        let service_state = Self {
            infra,
            crypto,
            file_service,
            file_service_redirect,
            connection_sync,
            webhook: webhook_service,
            endpoint_policy,
            engine,
            detection: Coordinator::new(),
            assistant: Coordinator::new(),
            shutdown: CancellationToken::new(),
            health_cache: HealthCache::new(&health_config, health_checkers),
            password,
            session_keys,
            sign_in,
            oidc,
            user_agent_parser,
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
        workers.spawn(EventOutboxDrainer::new(self.infra.clone()));
        workers.spawn(DetectionOutboxDrainer::new(
            self.infra.clone(),
            self.detection.clone(),
        ));
        workers.spawn(DetectionWorker::new(
            self.infra.clone(),
            self.engine.clone(),
            RunBlobStore::from_ref(self),
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

/// Derives [`FromRef`] for a per-resource domain service by building it over the
/// Postgres client from the shared [`Infra`]. Each service holds only the client
/// and acquires its own connection per call, so construction is a cheap clone.
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_domain {
    ($($t:ty),+ $(,)?) => {$(
        impl axum::extract::FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                <$t>::new(state.infra.postgres.clone())
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

// Stored fields get their `FromRef` from the `#[derive(FromRef)]` on
// `ServiceState` (each is a field clone); the fields skipped there
// (`detection`/`assistant` coordinators and the `oidc` config) are read by the
// composed impls below instead of extracted directly.

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
// detection `Coordinator` — so it needs a hand-written `FromRef` rather than the
// compose-from-`Infra`-alone macro above.
impl axum::extract::FromRef<ServiceState> for DetectionQueue {
    fn from_ref(state: &ServiceState) -> Self {
        DetectionQueue::new(state.infra.clone(), state.detection.clone())
    }
}

// `AssistantQueue` likewise composes from `Infra` and the shared assistant
// `Coordinator`, so it needs a hand-written `FromRef`.
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

// `OidcService` is composed per request from the stored startup configuration
// (`OidcConfigured`, holding the expensive HTTP client and parsed providers) plus
// the ambient collaborators its flow drives — no field of it is rebuilt here, only
// `Arc`-backed handles are cloned.
impl axum::extract::FromRef<ServiceState> for OidcService {
    fn from_ref(state: &ServiceState) -> Self {
        state.oidc.service(
            state.infra.postgres.clone(),
            state.infra.nats.clone(),
            AuthIssuer::from_ref(state),
            AccountProvisioner,
        )
    }
}

// Stateless unit services, holding nothing and acting on the connection passed to
// each method:
impl_di_unit!(AccountProvisioner);

// Per-resource domain services built over the Postgres client alone:
impl_di_domain!(
    domain::AccountService,
    domain::WorkspaceInviteService,
    domain::WorkspaceMemberService,
    domain::WorkspacePolicyService,
    domain::WorkspacePipelineService,
    domain::WorkspaceService,
);

// `AccountIdentityService` also holds the password service (to strength-check,
// hash, and verify passwords), so it needs a hand-written `FromRef`.
impl axum::extract::FromRef<ServiceState> for domain::AccountIdentityService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::AccountIdentityService::new(state.infra.postgres.clone(), state.password.clone())
    }
}

// `AccountApiTokenService` also holds the auth issuer (to sign a new token's
// one-time JWT), so it needs a hand-written `FromRef`.
impl axum::extract::FromRef<ServiceState> for domain::AccountApiTokenService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::AccountApiTokenService::new(
            state.infra.postgres.clone(),
            AuthIssuer::from_ref(state),
        )
    }
}

// `AccountNotificationService` also holds the notification emitter (to broadcast
// the unread count after a mark-read), so it needs a hand-written `FromRef`.
impl axum::extract::FromRef<ServiceState> for domain::AccountNotificationService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::AccountNotificationService::new(
            state.infra.postgres.clone(),
            NotificationEmitter::from_ref(state),
        )
    }
}

// `WorkspaceWebhookService` also holds the crypto service (to mint and decrypt
// the signing secret) and the delivery client (to send a test), so it needs a
// hand-written `FromRef`.
impl axum::extract::FromRef<ServiceState> for domain::WorkspaceWebhookService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::WorkspaceWebhookService::new(
            state.infra.postgres.clone(),
            state.crypto.clone(),
            state.webhook.clone(),
        )
    }
}

// `WorkspaceThreadService` also holds the assistant queue (to wake the reply
// drainer when a comment addresses the assistant), so it needs a hand-written
// `FromRef`.
impl axum::extract::FromRef<ServiceState> for domain::WorkspaceThreadService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::WorkspaceThreadService::new(
            state.infra.postgres.clone(),
            AssistantQueue::from_ref(state),
        )
    }
}

// `WorkspaceProviderService` and `WorkspaceConnectionService` also hold the crypto
// service (to encrypt and decrypt the provider/connection config) and the endpoint
// policy (to validate custom endpoints at write time), so they need hand-written
// `FromRef`s.
impl axum::extract::FromRef<ServiceState> for domain::WorkspaceProviderService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::WorkspaceProviderService::new(
            state.infra.postgres.clone(),
            state.crypto.clone(),
            state.endpoint_policy,
        )
    }
}

impl axum::extract::FromRef<ServiceState> for domain::WorkspaceConnectionService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::WorkspaceConnectionService::new(
            state.infra.postgres.clone(),
            state.crypto.clone(),
            state.endpoint_policy,
        )
    }
}

// `WorkspaceDocumentService` also holds the engine (to resolve list-filter format
// and modality tokens), so it needs a hand-written `FromRef`.
impl axum::extract::FromRef<ServiceState> for domain::WorkspaceDocumentService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::WorkspaceDocumentService::new(state.infra.postgres.clone(), state.engine.clone())
    }
}

// `WorkspaceDetectionService` also holds the detection queue (to broadcast the
// initial status and wake the outbox drainer after a create), so it needs a
// hand-written `FromRef`.
impl axum::extract::FromRef<ServiceState> for domain::WorkspaceDetectionService {
    fn from_ref(state: &ServiceState) -> Self {
        domain::WorkspaceDetectionService::new(
            state.infra.postgres.clone(),
            DetectionQueue::from_ref(state),
        )
    }
}

// `Domain` bundles the domain services; resolved by composing each from the
// state, the way `Infra` bundles the ambient clients.
impl axum::extract::FromRef<ServiceState> for domain::Domain {
    fn from_ref(state: &ServiceState) -> Self {
        domain::Domain {
            accounts: domain::AccountService::from_ref(state),
            account_api_tokens: domain::AccountApiTokenService::from_ref(state),
            account_identities: domain::AccountIdentityService::from_ref(state),
            account_notifications: domain::AccountNotificationService::from_ref(state),
            connections: domain::WorkspaceConnectionService::from_ref(state),
            detections: domain::WorkspaceDetectionService::from_ref(state),
            documents: domain::WorkspaceDocumentService::from_ref(state),
            invites: domain::WorkspaceInviteService::from_ref(state),
            members: domain::WorkspaceMemberService::from_ref(state),
            pipelines: domain::WorkspacePipelineService::from_ref(state),
            policies: domain::WorkspacePolicyService::from_ref(state),
            providers: domain::WorkspaceProviderService::from_ref(state),
            threads: domain::WorkspaceThreadService::from_ref(state),
            webhooks: domain::WorkspaceWebhookService::from_ref(state),
            workspaces: domain::WorkspaceService::from_ref(state),
        }
    }
}
