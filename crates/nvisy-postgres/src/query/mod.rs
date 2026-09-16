//! Database query repositories for all entities in the system.
//!
//! This module contains repository implementations that provide high-level
//! database operations for all entities, encapsulating common patterns
//! and providing type-safe interfaces.
//!
//! # Pagination
//!
//! Queries that may return large result sets support two pagination strategies:
//! - [`CursorPagination`]: Preferred for API endpoints, infinite scroll, and large datasets
//! - [`OffsetPagination`]: For admin dashboards or when random page access is needed
//!
//! [`CursorPagination`]: crate::types::CursorPagination
//! [`OffsetPagination`]: crate::types::OffsetPagination

mod account_api_tokens;
mod account_identities;
mod account_notifications;
mod accounts;
mod analytics;
mod blob_pointers;
mod pipeline_references;
mod search;
mod workspace_activities;
mod workspace_assistant_jobs;
mod workspace_blobs;
mod workspace_connection_schedule;
mod workspace_connection_syncs;
mod workspace_connections;
mod workspace_detection_jobs;
mod workspace_detection_policy_versions;
mod workspace_detections;
mod workspace_documents;
mod workspace_event_outbox;
mod workspace_invites;
mod workspace_members;
mod workspace_pipelines;
mod workspace_policies;
mod workspace_policy_versions;
mod workspace_providers;
mod workspace_redactions;
mod workspace_review_comments;
mod workspace_review_events;
mod workspace_reviews;
mod workspace_webhooks;
mod workspaces;

pub use account_api_tokens::{AccountApiTokenRepository, ApiTokenCursor};
pub use account_identities::{
    AccountIdentityRepository, DeleteIdentityOutcome, LinkIdentityOutcome,
};
pub use account_notifications::{AccountNotificationRepository, NotificationCursor};
pub use accounts::AccountRepository;
pub use analytics::{
    AnalyticsSnapshot, DetectionDayPoint, DetectionDurations, DetectionStatusCount, StorageByKind,
    UsageByModel, WorkspaceAnalyticsRepository,
};
pub use blob_pointers::{
    clear_expired_detection_audits, clear_expired_detection_intermediates,
    clear_expired_review_audits,
};
pub use pipeline_references::PipelineReferenceRepository;
pub use workspace_activities::{ActivityCursor, ActivityFilter, WorkspaceActivityRepository};
pub use workspace_assistant_jobs::AssistantJobOutboxRepository;
pub use workspace_blobs::{ReclaimableBlob, WorkspaceBlobRepository};
pub use workspace_connection_schedule::WorkspaceConnectionScheduleRepository;
pub use workspace_connection_syncs::{ConnectionSyncCursor, WorkspaceConnectionSyncRepository};
pub use workspace_connections::{
    ConnectionCursor, ScheduledConnection, WorkspaceConnectionRepository,
};
pub use workspace_detection_jobs::DetectionJobOutboxRepository;
pub use workspace_detection_policy_versions::DetectionPolicyVersionRepository;
pub use workspace_detections::{
    DetectionCursor, DetectionDocuments, DetectionListRow, WorkspaceDetectionRepository,
};
pub use workspace_documents::{
    DocumentCursor, DocumentWithBlob, ImportedDocumentRef, WorkspaceDocumentRepository,
};
pub use workspace_event_outbox::EventOutboxRepository;
pub use workspace_invites::{InviteCursor, WorkspaceInviteRepository};
pub use workspace_members::{
    AccountWorkspaceCursor, WorkspaceMemberCursor, WorkspaceMemberRepository,
};
pub use workspace_pipelines::{PipelineCursor, WorkspacePipelineRepository};
pub use workspace_policies::{PolicyCursor, PolicyWithVersion, WorkspacePolicyRepository};
pub use workspace_policy_versions::WorkspacePolicyVersionRepository;
pub use workspace_providers::{ProviderCursor, WorkspaceProviderRepository};
pub use workspace_redactions::{RedactionCursor, WorkspaceRedactionRepository};
pub use workspace_review_comments::WorkspaceReviewCommentRepository;
pub use workspace_review_events::{TimelineCursor, TimelineSource, WorkspaceReviewEventRepository};
pub use workspace_reviews::{
    AssignmentOutcome, DocumentReviewCursor, ReviewEventCursor, WithActor, WithReviewers,
    WorkspaceReviewRepository,
};
pub use workspace_webhooks::{WebhookCursor, WorkspaceWebhookRepository};
pub use workspaces::WorkspaceRepository;
