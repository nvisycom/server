//! Enhanced HTTP request extractors with improved error handling and validation.
//!
//! This module provides a comprehensive suite of custom Axum extractors that enhance
//! the default functionality with better error messages, validation, logging, and
//! type safety. All extractors are designed to be drop-in replacements for their
//! standard Axum counterparts while providing additional features.
//!
//! # Features
//!
//! - **Enhanced Error Handling**: Detailed, user-friendly error messages with context
//! - **Structured Logging**: Comprehensive tracing support for debugging and monitoring
//! - **Type Safety**: Strong typing with compile-time guarantees
//! - **Validation**: Automatic data validation with detailed error reporting
//! - **Performance**: Request extension caching to avoid repeated extractions
//! - **Security**: Built-in authentication and authorization with database verification
//!
//! # Extractor Categories
//!
//! ## Authentication & Authorization
//!
//! - [`AuthHeader`] - JWT token extraction and validation
//! - [`AuthClaims`] - JWT claims with application-specific fields
//! - [`AuthState`] - Complete authentication state with database verification
//! - [`AuthorizationProvider`] - Trait for types that can perform authorization
//! - [`Permission`] - Standard project permission levels
//! - [`AuthResult`] - Result of authorization checks
//!
//! ## Request Data Extraction
//!
//! - [`Json`] - Enhanced JSON deserialization with better error messages
//! - [`ValidateJson`] - JSON extraction with automatic validation
//! - [`Path`] - Path parameter extraction with detailed error context
//! - [`Query`] - Query parameter extraction with enhanced error messages
//! - [`Form`] - Form data extraction with improved error handling
//! ## Connection & Metadata
//!
//! - [`AppConnectInfo`] - Client connection information with IP analysis
//! - [`Version`] - API version extraction and validation

mod auth;
mod connection_info;
mod reject;
mod version;

pub use crate::authorize;
pub use crate::extract::auth::{
    AuthClaims, AuthHeader, AuthProvider, AuthResult, AuthState, Permission,
    TRACING_TARGET_AUTHENTICATION, TRACING_TARGET_AUTHORIZATION,
};
pub use crate::extract::connection_info::AppConnectInfo;
pub use crate::extract::reject::{Form, Json, Path, Query, ValidateJson};
pub use crate::extract::version::Version;
