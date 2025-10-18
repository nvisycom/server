//! Database enumeration types for type-safe queries.
//!
//! This module provides strongly-typed enumerations that correspond to PostgreSQL ENUM types
//! defined in the database schema. Each enumeration provides serialization support for APIs
//! and database integration through Diesel.

// Account-related enumerations
pub mod action_token_type;
pub mod api_token_type;

// Project-related enumerations
pub mod integration_status;
pub mod invite_status;
pub mod project_role;
pub mod project_status;
pub mod project_visibility;

// Document-related enumerations
pub mod document_status;
pub mod file_type;
pub mod processing_status;
pub mod require_mode;
pub mod virus_scan_status;

pub use action_token_type::ActionTokenType;
pub use api_token_type::ApiTokenType;
pub use document_status::DocumentStatus;
pub use file_type::FileType;
pub use integration_status::IntegrationStatus;
pub use invite_status::InviteStatus;
pub use processing_status::ProcessingStatus;
pub use project_role::ProjectRole;
pub use project_status::ProjectStatus;
pub use project_visibility::ProjectVisibility;
pub use require_mode::RequireMode;
pub use virus_scan_status::VirusScanStatus;
