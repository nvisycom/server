//! Enhanced HTTP request extractors with improved error handling and validation.
//!
//! This module provides a comprehensive suite of custom Axum extractors that enhance
//! the default functionality with better error messages, validation, logging, and
//! type safety. All extractors are designed to be drop-in replacements for their
//! standard Axum counterparts while providing additional features.

mod auth;
mod connection_info;
mod reject;
mod typed_header;
mod version;

pub use crate::extract::auth::{
    AuthClaims, AuthHeader, AuthProvider, AuthResult, AuthState, Permission,
    TRACING_TARGET_AUTHENTICATION, TRACING_TARGET_AUTHORIZATION,
};
pub use crate::extract::connection_info::{AppConnectInfo, ClientIp};
pub use crate::extract::reject::{Form, Json, Path, Query, ValidateJson};
pub use crate::extract::typed_header::TypedHeader;
pub use crate::extract::version::Version;
