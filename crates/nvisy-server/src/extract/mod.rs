//! Request extractors with improved error handling and validation.
//!
//! This module provides a comprehensive suite of custom Axum extractors that enhance
//! the default functionality with better error messages, validation, logging, and
//! type safety. All extractors are designed to be drop-in replacements for their
//! standard Axum counterparts while providing additional features.

mod auth;
mod avatar_upload;
mod idempotency_key;
mod reject;
mod security_context;
mod valid;
mod version;
mod workspace_context;

pub use crate::extract::auth::{
    AuthClaims, AuthState, AuthTransport, Authorized, CSRF_COOKIE_NAME, CSRF_HEADER_NAME,
    OptionalAuth, Permission, RequiredPermission, SESSION_COOKIE_NAME, SessionToken, markers,
};
pub use crate::extract::avatar_upload::AvatarUpload;
pub use crate::extract::idempotency_key::IdempotencyKey;
pub use crate::extract::reject::{Form, Json, Multipart, Path, Query};
pub use crate::extract::security_context::SecurityContext;
pub use crate::extract::valid::{ValidateJson, validators};
pub use crate::extract::version::Version;
pub use crate::extract::workspace_context::WorkspaceContext;
