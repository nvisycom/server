//! Response types for HTTP handlers.

pub mod account;
pub mod authentication;
pub mod document;
pub mod document_file;
pub mod document_version;
pub mod monitor;
pub mod project;
pub mod project_invite;
pub mod project_member;

mod error;

pub use error::ErrorResponse;
