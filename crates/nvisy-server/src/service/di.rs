//! Dependency-injection wiring: the [`FromRef`] impls that resolve each service
//! from [`ServiceState`] for Axum's [`State`] extractor.
//!
//! Stored fields get their `FromRef` from the `#[derive(FromRef)]` on
//! [`ServiceState`] (each is a field clone). This module covers the rest:
//! - ambient clients cloned out of the shared [`Infra`] ([`impl_di_infra`]);
//! - stateless services composed from `Infra` ([`impl_di_compose`]) or holding
//!   nothing ([`impl_di_unit`]);
//! - per-resource domain services built over the pooled client
//!   ([`impl_di_domain`] for the Postgres-only ones, [`impl_di_new`] for those
//!   that also hold one or two extra handles);
//! - and a handful of genuinely-distinct composites written by hand.
//!
//! [`FromRef`]: axum::extract::FromRef
//! [`State`]: axum::extract::State

use axum::extract::FromRef;

use crate::service::{
    AccountProvisioner, ArtifactReader, ArtifactWriter, AssistantQueue, AuthIssuer, AvatarService,
    DetectionQueue, ExternalObjectStore, Infra, NotificationEmitter, OidcService, ServiceState,
    WebhookEmitter,
};

/// Derives [`FromRef`] for a single ambient client by cloning it out of the
/// shared [`Infra`].
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_infra {
    ($($f:ident: $t:ty),+ $(,)?) => {$(
        impl FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                state.infra.$f.clone()
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
        impl FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                let ctor: fn(Infra) -> $t = $ctor;
                ctor(state.infra.clone())
            }
        }
    )+};
}

/// Derives [`FromRef`] for a stateless unit service that holds nothing and
/// operates entirely on the connection passed to each method.
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_unit {
    ($($t:ident),+ $(,)?) => {$(
        impl FromRef<ServiceState> for $t {
            fn from_ref(_state: &ServiceState) -> Self {
                $t
            }
        }
    )+};
}

/// Derives [`FromRef`] for a per-resource domain service built over the Postgres
/// client alone. Each holds only the client and acquires its own connection per
/// call, so construction is a cheap clone.
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_domain {
    ($($t:ty),+ $(,)?) => {$(
        impl FromRef<ServiceState> for $t {
            fn from_ref(state: &ServiceState) -> Self {
                <$t>::new(state.infra.postgres.clone())
            }
        }
    )+};
}

/// Derives [`FromRef`] for a service constructed from the pooled Postgres client
/// plus one or more extra handles. Collapses the otherwise-identical hand-written
/// impls for the domain services that hold an ambient dependency.
///
/// Each arm is `Type => |state| dep, ...`: the leading `|state|` binds the state
/// reference (once per arm) so every dependency expression can resolve against it
/// — a field clone (`state.crypto.clone()`), another service's `FromRef`
/// (`AuthIssuer::from_ref(state)`), or a `Copy` field (`state.endpoint_policy`).
/// The binding is threaded through the macro rather than hidden inside it so the
/// expressions are hygienic.
///
/// [`FromRef`]: axum::extract::FromRef
macro_rules! impl_di_new {
    ($($t:ty => |$state:ident| $($dep:expr),+ $(,)?);+ $(;)?) => {$(
        impl FromRef<ServiceState> for $t {
            fn from_ref($state: &ServiceState) -> Self {
                <$t>::new($($dep),+)
            }
        }
    )+};
}

// The ambient clients, resolved from the shared `Infra`:
impl_di_infra!(
    postgres: nvisy_postgres::PgClient,
    nats: nvisy_nats::NatsClient,
    blobs: nvisy_s3::BlobStore,
);

// Stateless services, composed from `Infra` on extraction:
impl_di_compose!(
    AvatarService => AvatarService::new,
    WebhookEmitter => WebhookEmitter::new,
    NotificationEmitter => NotificationEmitter::new,
);

// Stateless unit services, holding nothing and acting on the connection passed to
// each method:
impl_di_unit!(AccountProvisioner);

// Per-resource domain services built over the Postgres client alone:
impl_di_domain!(
    crate::domain::AccountService,
    crate::domain::WorkspaceInviteService,
    crate::domain::WorkspaceMemberService,
    crate::domain::WorkspacePolicyService,
    crate::domain::WorkspacePipelineService,
    crate::domain::WorkspaceService,
);

// Per-resource domain services that also hold one or two ambient handles beyond
// the pooled client — each dependency resolved from the state:
impl_di_new! {
    // The password service (strength-check, hash, verify).
    crate::domain::AccountIdentityService => |state| state.infra.postgres.clone(), state.password.clone();
    // The auth issuer (sign a new token's one-time JWT).
    crate::domain::AccountApiTokenService =>
        |state| state.infra.postgres.clone(), AuthIssuer::from_ref(state);
    // The notification emitter (broadcast the unread count after a mark-read).
    crate::domain::AccountNotificationService =>
        |state| state.infra.postgres.clone(), NotificationEmitter::from_ref(state);
    // Crypto (mint/decrypt the signing secret) and the delivery client (send a test).
    crate::domain::WorkspaceWebhookService =>
        |state| state.infra.postgres.clone(), state.crypto.clone(), state.webhook.clone();
    // The assistant queue (wake the reply drainer when a comment addresses it).
    crate::domain::WorkspaceReviewService =>
        |state| state.infra.postgres.clone(), AssistantQueue::from_ref(state);
    // Crypto (encrypt/decrypt config) and the endpoint policy (validate endpoints).
    crate::domain::WorkspaceProviderService =>
        |state| state.infra.postgres.clone(), state.crypto.clone(), state.endpoint_policy;
    crate::domain::WorkspaceConnectionService =>
        |state| state.infra.postgres.clone(), state.crypto.clone(), state.endpoint_policy;
    // The engine (resolve list-filter format/modality tokens).
    crate::domain::WorkspaceDocumentService => |state| state.infra.postgres.clone(), state.engine.clone();
    // The detection queue (broadcast status and wake the drainer after a create).
    crate::domain::WorkspaceDetectionService =>
        |state| state.infra.postgres.clone(), DetectionQueue::from_ref(state);
    // The blob writer/reader both encrypt/decrypt blob content, so each composes
    // from `Infra` plus the crypto service.
    ArtifactWriter => |state| state.infra.clone(), state.crypto.clone();
    ArtifactReader => |state| state.infra.clone(), state.crypto.clone();
}

// `DetectionQueue` composes from two singletons — `Infra` and the shared
// detection `Coordinator` — so it needs a hand-written `FromRef` rather than the
// compose-from-`Infra`-alone macro above.
impl FromRef<ServiceState> for DetectionQueue {
    fn from_ref(state: &ServiceState) -> Self {
        DetectionQueue::new(state.infra.clone(), state.detection.clone())
    }
}

// `AssistantQueue` likewise composes from `Infra` and the shared assistant
// `Coordinator`, so it needs a hand-written `FromRef`.
impl FromRef<ServiceState> for AssistantQueue {
    fn from_ref(state: &ServiceState) -> Self {
        AssistantQueue::new(state.infra.clone(), state.assistant.clone())
    }
}

// `ExternalObjectStore` holds only the deployment's endpoint policy:
impl FromRef<ServiceState> for ExternalObjectStore {
    fn from_ref(state: &ServiceState) -> Self {
        ExternalObjectStore::new(state.endpoint_policy)
    }
}

// `AuthIssuer` composes from two security fields — the JWT signing keys and the
// user-agent parser (for session display names):
impl FromRef<ServiceState> for AuthIssuer {
    fn from_ref(state: &ServiceState) -> Self {
        AuthIssuer::new(state.session_keys.clone(), state.user_agent_parser.clone())
    }
}

// `OidcService` is composed per request from the stored startup configuration
// (`OidcConfigured`, holding the expensive HTTP client and parsed providers) plus
// the ambient collaborators its flow drives — no field of it is rebuilt here, only
// `Arc`-backed handles are cloned.
impl FromRef<ServiceState> for OidcService {
    fn from_ref(state: &ServiceState) -> Self {
        state.oidc.service(
            state.infra.postgres.clone(),
            state.infra.nats.clone(),
            AuthIssuer::from_ref(state),
            AccountProvisioner,
        )
    }
}
