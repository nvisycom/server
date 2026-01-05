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

pub mod account;
pub mod account_action_token;
pub mod account_api_token;
pub mod account_notification;

pub mod document;
pub mod document_annotation;
pub mod document_chunk;
pub mod document_comment;
pub mod document_file;

pub mod workspace;
pub mod workspace_activity;
pub mod workspace_integration;
pub mod workspace_integration_run;
pub mod workspace_invite;
pub mod workspace_member;
pub mod workspace_webhook;

pub use account::AccountRepository;
pub use account_action_token::AccountActionTokenRepository;
pub use account_api_token::AccountApiTokenRepository;
pub use account_notification::AccountNotificationRepository;
pub use document::DocumentRepository;
pub use document_annotation::DocumentAnnotationRepository;
pub use document_chunk::DocumentChunkRepository;
pub use document_comment::DocumentCommentRepository;
pub use document_file::DocumentFileRepository;
pub use workspace::WorkspaceRepository;
pub use workspace_activity::WorkspaceActivityRepository;
pub use workspace_integration::WorkspaceIntegrationRepository;
pub use workspace_integration_run::WorkspaceIntegrationRunRepository;
pub use workspace_invite::WorkspaceInviteRepository;
pub use workspace_member::WorkspaceMemberRepository;
pub use workspace_webhook::WorkspaceWebhookRepository;
