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
mod account_notification;
mod analytics;
mod chat_message;
mod chat_session;
mod event_outbox;
mod pipeline_reference;
mod search;
mod workspace;
mod workspace_activity;
mod workspace_connection;
mod workspace_connection_schedule;
mod workspace_connection_sync;
mod workspace_detection;
mod workspace_file;
mod workspace_invite;
mod workspace_member;
mod workspace_pipeline;
mod workspace_policy;
mod workspace_redaction;
mod workspace_webhook;

pub use account::AccountRepository;
pub use account_api_token::AccountApiTokenRepository;
pub use account_notification::AccountNotificationRepository;
pub use analytics::{
    AnalyticsSnapshot, DetectionDayPoint, DetectionDurations, DetectionStatusCount, StorageByKind,
    UsageByModel, WorkspaceAnalyticsRepository,
};
pub use chat_message::{AppendSessionUpdate, ChatMessageRepository};
pub use chat_session::ChatSessionRepository;
pub use event_outbox::EventOutboxRepository;
pub use pipeline_reference::PipelineReferenceRepository;
pub use workspace::WorkspaceRepository;
pub use workspace_activity::{ActivityFilter, WorkspaceActivityRepository};
pub use workspace_connection::{ScheduledConnection, WorkspaceConnectionRepository};
pub use workspace_connection_schedule::WorkspaceConnectionScheduleRepository;
pub use workspace_connection_sync::WorkspaceConnectionSyncRepository;
pub use workspace_detection::{DetectionFiles, DetectionListRow, WorkspaceDetectionRepository};
pub use workspace_file::{ExpiredFileRef, ImportedFileRef, WorkspaceFileRepository};
pub use workspace_invite::WorkspaceInviteRepository;
pub use workspace_member::WorkspaceMemberRepository;
pub use workspace_pipeline::WorkspacePipelineRepository;
pub use workspace_policy::WorkspacePolicyRepository;
pub use workspace_redaction::WorkspaceRedactionRepository;
pub use workspace_webhook::WorkspaceWebhookRepository;
