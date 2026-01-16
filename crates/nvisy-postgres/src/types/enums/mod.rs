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

// Document-related enumerations
pub mod annotation_type;
pub mod content_segmentation;
pub mod file_source;
pub mod processing_status;
pub mod require_mode;

// Chat-related enumerations
pub mod chat_session_status;
pub mod chat_tool_status;

pub use action_token_type::ActionTokenType;
pub use activity_type::{ActivityCategory, ActivityType};
pub use annotation_type::AnnotationType;
pub use api_token_type::ApiTokenType;
pub use chat_session_status::ChatSessionStatus;
pub use chat_tool_status::ChatToolStatus;
pub use content_segmentation::ContentSegmentation;
pub use file_source::FileSource;
pub use integration_status::IntegrationStatus;
pub use integration_type::IntegrationType;
pub use invite_status::InviteStatus;
pub use notification_event::NotificationEvent;
pub use processing_status::ProcessingStatus;
pub use require_mode::RequireMode;
pub use run_type::RunType;
pub use webhook_event::WebhookEvent;
pub use webhook_status::WebhookStatus;
pub use webhook_type::WebhookType;
pub use workspace_role::WorkspaceRole;
