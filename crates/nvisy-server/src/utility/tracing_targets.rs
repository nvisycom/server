//! Centralized tracing target constants for structured logging.
//!
//! This module defines all tracing target strings used throughout the crate,
//! providing a single source of truth for log categorization and filtering.
//! Using consistent targets enables fine-grained control over log output
//! via tracing subscriber filters.

/// Authentication-related operations including token validation and JWT processing.
pub const AUTHENTICATION: &str = "nvisy_server::authentication";

/// Authorization checks including permission verification and access control.
pub const AUTHORIZATION: &str = "nvisy_server::authorization";

/// Request metrics and performance monitoring.
pub const METRICS: &str = "nvisy_server::metrics";

/// Error recovery including middleware errors and request failures.
pub const RECOVERY_ERROR: &str = "nvisy_server::recovery::error";

/// Panic recovery including handler panics and service failures.
pub const RECOVERY_PANIC: &str = "nvisy_server::recovery::panic";

/// Password strength evaluation and validation.
pub const PASSWORD_STRENGTH: &str = "nvisy_server::password_strength";

/// Password hashing and verification operations.
pub const PASSWORD_HASHER: &str = "nvisy_server::password_hasher";

/// Session key management and JWT signing operations.
pub const SESSION_KEYS: &str = "nvisy_server::session_keys";

/// Health check caching and service availability monitoring.
pub const HEALTH_CACHE: &str = "nvisy_server::health_cache";
