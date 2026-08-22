//! Database enumeration types for type-safe queries.
//!
//! This module provides strongly-typed enumerations that correspond to PostgreSQL ENUM types
//! defined in the database schema. Each enumeration provides serialization support for APIs
//! and database integration through Diesel.

// Account-related enumerations
pub mod api_token_type;
pub mod notification_event;
pub mod outbox_status;

// Chat-related enumerations
pub mod chat_role;

// Connection-related enumerations
pub mod provider_type;

// Workspace-related enumerations
pub mod activity_type;
pub mod invite_status;
pub mod sync_deletion_policy;
pub mod sync_mode;
pub mod sync_status;
pub mod sync_trigger_type;
pub mod webhook_event;
pub mod webhook_status;
pub mod workspace_role;

// File-related enumerations
pub mod file_kind;

// Pipeline-related enumerations
pub mod pipeline_run_status;
pub mod pipeline_status;
pub mod pipeline_trigger_type;

pub use activity_type::ActivityType;
pub use api_token_type::ApiTokenType;
pub use chat_role::ChatRole;
pub use file_kind::FileKind;
pub use invite_status::InviteStatus;
pub use notification_event::NotificationEvent;
pub use outbox_status::OutboxStatus;
pub use pipeline_run_status::PipelineRunStatus;
pub use pipeline_status::PipelineStatus;
pub use pipeline_trigger_type::PipelineTriggerType;
pub use provider_type::ProviderType;
pub use sync_deletion_policy::SyncDeletionPolicy;
pub use sync_mode::SyncMode;
pub use sync_status::SyncStatus;
pub use sync_trigger_type::SyncTriggerType;
pub use webhook_event::WebhookEvent;
pub use webhook_status::WebhookStatus;
pub use workspace_role::WorkspaceRole;
