//! Per-resource domain services.
//!
//! Each service owns one resource's domain logic — the orchestration a handler
//! would otherwise inline: transactions, event emission, and multi-step rules.
//! Handlers stay thin, extracting a service and mapping its result to a response.
//!
//! A service holds the Postgres client and acquires its own connection per call,
//! so its methods take no connection and each is a self-contained transaction; a
//! service that also needs an ambient client (crypto, a queue, the engine) holds
//! it as a field too. Each is resolved per request from [`ServiceState`] via its
//! `FromRef` impl, so a handler extracts exactly the services it uses; every
//! field is an `Arc`-backed handle, so cloning is cheap.
//!
//! [`ServiceState`]: crate::service::ServiceState

pub mod input;
pub mod output;

mod account;
mod account_api_tokens;
mod account_identities;
mod account_notifications;
mod workspace_connections;
mod workspace_detections;
mod workspace_documents;
mod workspace_invites;
mod workspace_members;
mod workspace_pipelines;
mod workspace_policies;
mod workspace_providers;
mod workspace_reviews;
mod workspace_webhooks;
mod workspaces;

pub use account::AccountService;
pub use account_api_tokens::AccountApiTokenService;
pub use account_identities::{AccountIdentityService, ReauthVerified};
pub use account_notifications::AccountNotificationService;
pub use workspace_connections::WorkspaceConnectionService;
pub use workspace_detections::WorkspaceDetectionService;
pub use workspace_documents::WorkspaceDocumentService;
pub use workspace_invites::WorkspaceInviteService;
pub use workspace_members::WorkspaceMemberService;
pub use workspace_pipelines::WorkspacePipelineService;
pub use workspace_policies::WorkspacePolicyService;
pub use workspace_providers::WorkspaceProviderService;
pub use workspace_reviews::WorkspaceReviewService;
pub use workspace_webhooks::WorkspaceWebhookService;
pub use workspaces::WorkspaceService;
