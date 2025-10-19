#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// Tracing target constants for consistent logging across auth-related operations

/// Tracing target for authentication operations.
///
/// Used for logging JWT token validation, session verification, account lookup,
/// and other operations that verify user identity and session validity.
pub const TRACING_TARGET_AUTHENTICATION: &str = "nvisy_server::extract::authentication";

/// Tracing target for authorization operations.
///
/// Used for logging permission checks, role validation, project access control,
/// and other operations that determine what authenticated users can access.
pub const TRACING_TARGET_AUTHORIZATION: &str = "nvisy_server::extract::authorization";

pub mod extract;
pub mod handler;
pub mod middleware;
pub mod service;
