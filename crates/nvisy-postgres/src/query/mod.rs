//! Database query repositories for all entities in the system.
//!
//! This module contains repository implementations that provide high-level
//! database operations for all entities, encapsulating common patterns
//! and providing type-safe interfaces.
//!
//! List queries page by keyset cursor ([`CursorPagination`]).
//!
//! [`CursorPagination`]: crate::types::CursorPagination

mod account_api_tokens;
mod account_identities;
mod account_notifications;
mod accounts;
mod analytics;
mod blob_pointers;
mod outbox;
mod search;
mod workspace_activities;
mod workspace_blobs;
mod workspace_connection_schedule;
mod workspace_connection_syncs;
mod workspace_connections;
mod workspace_detection_policy_versions;
mod workspace_detections;
mod workspace_documents;
mod workspace_invites;
mod workspace_members;
mod workspace_policies;
mod workspace_policy_versions;
mod workspace_providers;
mod workspace_redactions;
mod workspace_review_comments;
mod workspace_review_events;
mod workspace_reviews;
mod workspace_webhooks;
mod workspaces;

pub use account_api_tokens::AccountApiTokenRepository;
pub use account_identities::{
    AccountIdentityRepository, DeleteIdentityOutcome, LinkIdentityOutcome,
};
pub use account_notifications::AccountNotificationRepository;
pub use accounts::AccountRepository;
pub use analytics::{
    AnalyticsSnapshot, DetectionDayPoint, DetectionDurations, DetectionStatusCount, StorageByKind,
    WorkspaceAnalyticsRepository,
};
pub use blob_pointers::{
    clear_expired_detection_audits, clear_expired_detection_intermediates,
    clear_expired_document_blobs, clear_expired_review_audits,
};
pub use outbox::{NewOutboxRow, OutboxRepository, OutboxRow};
pub use workspace_activities::{ActivityFilter, WorkspaceActivityRepository};
pub use workspace_blobs::{ReclaimableBlob, WorkspaceBlobRepository};
pub use workspace_connection_schedule::WorkspaceConnectionScheduleRepository;
pub use workspace_connection_syncs::WorkspaceConnectionSyncRepository;
pub use workspace_connections::{ScheduledConnection, WorkspaceConnectionRepository};
pub use workspace_detection_policy_versions::DetectionPolicyVersionRepository;
pub use workspace_detections::{
    DetectionDocuments, DetectionListRow, DetectionSummary, DetectionTreeRow, EditWriteOutcome,
    WorkspaceDetectionRepository,
};
pub use workspace_documents::{DocumentWithBlob, ImportedDocumentRef, WorkspaceDocumentRepository};
pub use workspace_invites::{InviteCursor, WorkspaceInviteRepository};
pub use workspace_members::{WorkspaceMemberCursor, WorkspaceMemberRepository};
pub use workspace_policies::{DerivedPolicy, PolicyWithVersion, WorkspacePolicyRepository};
pub use workspace_policy_versions::WorkspacePolicyVersionRepository;
pub use workspace_providers::WorkspaceProviderRepository;
pub use workspace_redactions::{DetectionRedactedOutput, WorkspaceRedactionRepository};
pub use workspace_review_comments::WorkspaceReviewCommentRepository;
pub use workspace_review_events::{
    TimelineCursor, TimelineEntry, TimelineSource, WorkspaceReviewEventRepository,
};
pub use workspace_reviews::{
    ApprovalOutcome, AssignmentOutcome, ReviewDetails, StatusOutcome, WorkspaceReviewRepository,
};
pub use workspace_webhooks::WorkspaceWebhookRepository;
pub use workspaces::WorkspaceRepository;
