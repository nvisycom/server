//! Database enumeration types for type-safe queries.
//!
//! This module provides strongly-typed enumerations that correspond to PostgreSQL ENUM types
//! defined in the database schema. Each enumeration provides serialization support for APIs
//! and database integration through Diesel.

// Account-related enumerations
pub mod action_token_type;
pub mod api_token_type;
pub mod notification_event;

// Workspace-related enumerations
pub mod activity_type;
pub mod integration_status;
pub mod integration_type;
pub mod invite_status;
pub mod run_type;
pub mod webhook_event;
pub mod webhook_status;
pub mod webhook_type;
pub mod workspace_role;

// File-related enumerations
pub mod annotation_type;
pub mod file_source;

// Pipeline-related enumerations
pub mod artifact_type;
pub mod pipeline_run_status;
pub mod pipeline_status;
pub mod pipeline_trigger_type;

pub use action_token_type::ActionTokenType;
pub use activity_type::{ActivityCategory, ActivityType};
pub use annotation_type::AnnotationType;
pub use api_token_type::ApiTokenType;
pub use artifact_type::ArtifactType;
pub use file_source::FileSource;
pub use integration_status::IntegrationStatus;
pub use integration_type::IntegrationType;
pub use invite_status::InviteStatus;
pub use notification_event::NotificationEvent;
pub use pipeline_run_status::PipelineRunStatus;
pub use pipeline_status::PipelineStatus;
pub use pipeline_trigger_type::PipelineTriggerType;
pub use run_type::RunType;
pub use webhook_event::WebhookEvent;
pub use webhook_status::WebhookStatus;
pub use webhook_type::WebhookType;
pub use workspace_role::WorkspaceRole;
