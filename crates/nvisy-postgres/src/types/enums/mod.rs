//! Database enumeration types for type-safe queries.
//!
//! This module provides strongly-typed enumerations that correspond to PostgreSQL ENUM types
//! defined in the database schema. Each enumeration provides serialization support for APIs
//! and database integration through Diesel.

// Account-related enumerations
pub mod action_token_type;
pub mod api_token_type;
pub mod notification_type;

// Project-related enumerations
pub mod activity_type;
pub mod integration_status;
pub mod integration_type;
pub mod invite_status;
pub mod project_role;
pub mod project_status;
pub mod project_visibility;
pub mod webhook_status;

// Document-related enumerations
pub mod content_segmentation;
pub mod document_status;
pub mod processing_status;
pub mod require_mode;
pub mod virus_scan_status;

pub use action_token_type::ActionTokenType;
pub use activity_type::{ActivityCategory, ActivityType};
pub use api_token_type::ApiTokenType;
pub use content_segmentation::ContentSegmentation;
pub use document_status::DocumentStatus;
pub use integration_status::IntegrationStatus;
pub use integration_type::IntegrationType;
pub use invite_status::InviteStatus;
pub use notification_type::NotificationType;
pub use processing_status::ProcessingStatus;
pub use project_role::ProjectRole;
pub use project_status::ProjectStatus;
pub use project_visibility::ProjectVisibility;
pub use require_mode::RequireMode;
pub use virus_scan_status::VirusScanStatus;
pub use webhook_status::WebhookStatus;
