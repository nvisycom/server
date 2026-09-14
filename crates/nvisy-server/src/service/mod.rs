//! Application state and dependency injection.

mod account_provisioner;
mod auth_flow;
mod auth_issuer;
mod avatar;
mod crypto;
mod di;
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
pub use nvisy_object_store::client::ExternalObjectStore;
pub use nvisy_s3::S3Config;
use nvisy_webhook::WebhookService;
use tokio_util::sync::CancellationToken;

use crate::Result;
use crate::args::ServiceArgs;
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

/// Tracing target for service-state initialization.
const TRACING_TARGET: &str = "nvisy_server::service";

/// Application state.
///
/// Used for the [`State`] extraction (dependency injection).
///
/// A field is stored when it carries live state or wraps a config-loaded
/// resource; each such field gets its extractor from the `#[derive(FromRef)]`
/// (a field clone), except the three marked `#[from_ref(skip)]` (the two wake
/// coordinators, which also share a type, and the OIDC config, which is only ever
/// composed into [`OidcService`]). The services that are pure compositions of
/// these — [`AvatarService`], [`RunBlobStore`], [`DetectionQueue`], the domain
/// services, [`OidcService`], and so on — are not fields; they are built on demand
/// in their `FromRef` impls (see the `di` module), a cheap move over `Arc`-backed
/// handles.
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
    /// Initializes application state from the aggregated [`ServiceArgs`] and a
    /// caller-provided [`WebhookService`].
    ///
    /// Connects to all external services and loads required resources. The
    /// webhook client is injected rather than derived from config, so a caller
    /// can supply any implementation (the first-party CLI uses the reqwest-based
    /// one).
    pub async fn from_config(args: ServiceArgs, webhook_service: WebhookService) -> Result<Self> {
        let infra = Infra::from_config(args.postgres, args.nats, args.s3).await?;

        // Reconcile every JetStream stream once, up front, so the publishers and
        // subscribers built per use later are cheap handles that assume their
        // stream already exists.
        ensure_streams(&infra.nats).await?;

        let crypto = CryptoService::from_config(&args.crypto).await?;
        let engine = EngineService::from_config(args.engine).await?;
        let session_keys = SessionKeys::from_config(&args.session_keys).await?;
        let oidc = OidcConfigured::from_config(&args.oidc)?;

        // Session cookies without `Secure` are only safe over plain HTTP on a
        // trusted network (local development or trusted-network self-hosting); a
        // browser will not even store them over HTTPS. Warn loudly so an
        // accidental production misconfiguration is visible.
        if !args.cookie.secure {
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
        let (file_service, file_service_redirect) = args.file_service.build()?;
        let endpoint_policy = args.integration.endpoint_policy;
        let connection_sync = ConnectionSyncService::new(
            infra.clone(),
            crypto.clone(),
            ExternalObjectStore::new(endpoint_policy),
            file_service.clone(),
            args.integration.import_concurrency,
            args.integration.export_concurrency,
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
            health_cache: HealthCache::new(&args.health, health_checkers),
            password,
            session_keys,
            sign_in,
            oidc,
            user_agent_parser,
            upload: args.upload,
            cookie: args.cookie,
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
