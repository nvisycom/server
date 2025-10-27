//! Middleware for `axum::Router` and HTTP request processing.
//!
//! This module provides a comprehensive set of middleware for:
//! - Authentication and authorization
//! - Security (CORS, headers, body limits)
//! - Observability (metrics, tracing, request IDs)
//! - Error handling (panics, timeouts, service errors)
//! - Rate limiting
//! - OpenAPI documentation
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use axum::Router;
//! use nvisy_server::middleware::{RouterExt, CorsConfig, SecurityHeadersConfig};
//!
//! let app = Router::new()
//!     .with_error_handling_layer(Duration::from_secs(30))
//!     .with_observability_layer()
//!     .with_default_security_layer()
//!     .with_metrics_layer();
//! ```

mod auth;
mod error_handling;
mod extensions;
mod observability;
pub mod open_api;
mod rate_limiting;
pub mod security;

// Re-export authentication middleware
pub use auth::{refresh_token_middleware, require_admin, require_authentication};
// Internal re-exports used by extension trait implementations
// Re-export extension traits (main API for applying middleware)
pub use extensions::RouterExt;
// Re-export OpenAPI
pub use open_api::{OpenApiConfig, RouterOpenApiExt};
// Re-export security configuration (needed for custom config)
pub use security::{CorsConfig, SecurityHeadersConfig};
