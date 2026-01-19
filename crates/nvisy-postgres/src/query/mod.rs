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
mod account_action_token;
mod account_api_token;
mod account_notification;

mod file;
mod file_annotation;
mod file_chunk;

mod pipeline;
mod pipeline_run;

mod workspace;
mod workspace_activity;
mod workspace_integration;
mod workspace_integration_run;
mod workspace_invite;
mod workspace_member;
mod workspace_webhook;

pub use account::AccountRepository;
pub use account_action_token::AccountActionTokenRepository;
pub use account_api_token::AccountApiTokenRepository;
pub use account_notification::AccountNotificationRepository;

pub use file::FileRepository;
pub use file_annotation::FileAnnotationRepository;
pub use file_chunk::FileChunkRepository;

pub use pipeline::PipelineRepository;
pub use pipeline_run::PipelineRunRepository;

pub use workspace::WorkspaceRepository;
pub use workspace_activity::WorkspaceActivityRepository;
pub use workspace_integration::WorkspaceIntegrationRepository;
pub use workspace_integration_run::WorkspaceIntegrationRunRepository;
pub use workspace_invite::WorkspaceInviteRepository;
pub use workspace_member::WorkspaceMemberRepository;
pub use workspace_webhook::WorkspaceWebhookRepository;
