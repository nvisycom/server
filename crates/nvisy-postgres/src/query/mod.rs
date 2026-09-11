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

mod account;
mod account_api_token;
mod account_identity;
mod account_notification;
mod analytics;
mod event_outbox;
mod pipeline_reference;
mod search;
mod workspace;
mod workspace_activity;
mod workspace_assignment;
mod workspace_assistant_job;
mod workspace_connection;
mod workspace_connection_schedule;
mod workspace_connection_sync;
mod workspace_detection;
mod workspace_detection_job;
mod workspace_file;
mod workspace_invite;
mod workspace_member;
mod workspace_pipeline;
mod workspace_policy;
mod workspace_provider;
mod workspace_redaction;
mod workspace_thread;
mod workspace_thread_anchor;
mod workspace_thread_comment;
mod workspace_thread_event;
mod workspace_webhook;

pub use account::AccountRepository;
pub use account_api_token::{AccountApiTokenRepository, ApiTokenCursor};
pub use account_identity::{AccountIdentityRepository, DeleteIdentityOutcome, LinkIdentityOutcome};
pub use account_notification::{AccountNotificationRepository, NotificationCursor};
pub use analytics::{
    AnalyticsSnapshot, DetectionDayPoint, DetectionDurations, DetectionStatusCount, StorageByKind,
    UsageByModel, WorkspaceAnalyticsRepository,
};
pub use event_outbox::EventOutboxRepository;
pub use pipeline_reference::PipelineReferenceRepository;
pub use workspace::WorkspaceRepository;
pub use workspace_activity::{ActivityCursor, ActivityFilter, WorkspaceActivityRepository};
pub use workspace_assignment::{
    AssignmentCursor, AssignmentListRow, CreateAssignmentOutcome, WorkspaceAssignmentRepository,
};
pub use workspace_assistant_job::AssistantJobOutboxRepository;
pub use workspace_connection::{
    ConnectionCursor, ScheduledConnection, WorkspaceConnectionRepository,
};
pub use workspace_connection_schedule::WorkspaceConnectionScheduleRepository;
pub use workspace_connection_sync::{ConnectionSyncCursor, WorkspaceConnectionSyncRepository};
pub use workspace_detection::{
    DetectionCursor, DetectionFiles, DetectionListRow, WorkspaceDetectionRepository,
};
pub use workspace_detection_job::DetectionJobOutboxRepository;
pub use workspace_file::{ExpiredFileRef, FileCursor, ImportedFileRef, WorkspaceFileRepository};
pub use workspace_invite::{InviteCursor, WorkspaceInviteRepository};
pub use workspace_member::{
    AccountWorkspaceCursor, WorkspaceMemberCursor, WorkspaceMemberRepository,
};
pub use workspace_pipeline::{PipelineCursor, WorkspacePipelineRepository};
pub use workspace_policy::{PolicyCursor, WorkspacePolicyRepository};
pub use workspace_provider::{ProviderCursor, WorkspaceProviderRepository};
pub use workspace_redaction::{RedactionCursor, WorkspaceRedactionRepository};
pub use workspace_thread::{ThreadCursor, WorkspaceThreadRepository};
pub use workspace_thread_anchor::WorkspaceThreadAnchorRepository;
pub use workspace_thread_comment::WorkspaceThreadCommentRepository;
pub use workspace_thread_event::{TimelineCursor, TimelineSource, WorkspaceThreadEventRepository};
pub use workspace_webhook::{WebhookCursor, WorkspaceWebhookRepository};
