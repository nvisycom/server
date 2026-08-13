//! Request extractors with improved error handling and validation.
//!
//! This module provides a comprehensive suite of custom Axum extractors that enhance
//! the default functionality with better error messages, validation, logging, and
//! type safety. All extractors are designed to be drop-in replacements for their
//! standard Axum counterparts while providing additional features.

mod auth;
mod avatar;
mod connection_info;
mod idempotency_key;
mod pg_connection;
mod reject;
mod typed_header;
mod version;
mod workspace_context;

pub use crate::extract::auth::{
    AuthClaims, AuthHeader, AuthProvider, AuthResult, AuthState, Permission,
};
pub use crate::extract::avatar::Avatar;
pub use crate::extract::connection_info::{AppConnectInfo, ClientIp};
pub use crate::extract::idempotency_key::IdempotencyKey;
pub use crate::extract::pg_connection::PgPool;
pub use crate::extract::reject::{Form, Json, Multipart, Path, Query, ValidateJson};
pub use crate::extract::typed_header::TypedHeader;
pub use crate::extract::version::Version;
pub use crate::extract::workspace_context::WorkspaceContext;
