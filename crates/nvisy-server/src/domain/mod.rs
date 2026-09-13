//! Per-resource domain services.
//!
//! Each service owns one resource's domain logic — the orchestration a handler
//! would otherwise inline: transactions, event emission, and multi-step rules.
//! Handlers stay thin, extracting a service and mapping its result to a response.
//!
//! A service holds the Postgres client and acquires its own connection per call,
//! so its methods take no connection and each is a self-contained transaction; a
//! service that also needs an ambient client (crypto, a queue, the engine) holds
//! it as a field too. [`Domain`] bundles the services the way
//! [`Infra`](crate::service::Infra) bundles the ambient clients; every field is an
//! `Arc`-backed handle, so cloning is cheap.

pub mod input;
pub mod output;

mod workspace_connections;
mod workspace_detections;
mod workspace_documents;
mod workspace_invites;
mod workspace_members;
mod workspace_pipelines;
mod workspace_policies;
mod workspace_providers;
mod workspace_threads;
mod workspace_webhooks;
mod workspaces;

pub use workspace_connections::WorkspaceConnectionService;
pub use workspace_detections::WorkspaceDetectionService;
pub use workspace_documents::WorkspaceDocumentService;
pub use workspace_invites::WorkspaceInviteService;
pub use workspace_members::WorkspaceMemberService;
pub use workspace_pipelines::WorkspacePipelineService;
pub use workspace_policies::WorkspacePolicyService;
pub use workspace_providers::WorkspaceProviderService;
pub use workspace_threads::WorkspaceThreadService;
pub use workspace_webhooks::WorkspaceWebhookService;
pub use workspaces::WorkspaceService;

/// The per-resource domain services handlers call.
#[derive(Clone)]
pub struct Domain {
    /// Workspace connection domain logic.
    pub connections: WorkspaceConnectionService,
    /// Workspace detection request-side domain logic.
    pub detections: WorkspaceDetectionService,
    /// Workspace document metadata domain logic.
    pub documents: WorkspaceDocumentService,
    /// Workspace invite domain logic.
    pub invites: WorkspaceInviteService,
    /// Workspace member domain logic.
    pub members: WorkspaceMemberService,
    /// Workspace pipeline domain logic.
    pub pipelines: WorkspacePipelineService,
    /// Workspace policy domain logic.
    pub policies: WorkspacePolicyService,
    /// Workspace provider domain logic.
    pub providers: WorkspaceProviderService,
    /// Workspace thread and comment domain logic.
    pub threads: WorkspaceThreadService,
    /// Workspace webhook domain logic.
    pub webhooks: WorkspaceWebhookService,
    /// Workspace CRUD domain logic.
    pub workspaces: WorkspaceService,
}
