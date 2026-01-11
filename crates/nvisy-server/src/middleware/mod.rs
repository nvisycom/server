//! Middleware for `axum::Router` and HTTP request processing.
//!
//! This module provides a comprehensive set of middleware for authentication,
//! authorization, security, observability, error recovery, and API documentation.
//! Each middleware category has its own extension trait for ergonomic composition.
//!
//! # Middleware Ordering
//!
//! The order in which middleware is applied matters significantly. Axum applies
//! layers in reverse order, meaning the last layer added wraps the outermost
//! request handling. The recommended ordering from outermost to innermost is:
//!
//! 1. **Recovery** - Catches panics and enforces timeouts at the outermost layer,
//!    ensuring all errors are properly handled regardless of where they occur.
//!
//! 2. **Observability** - Generates request IDs and adds tracing spans early,
//!    so all subsequent middleware and handlers are properly instrumented.
//!
//! 3. **Security** - Applies CORS, security headers, and body limits before
//!    any request processing occurs.
//!
//! 4. **Metrics** - Tracks request timing and categorization after security
//!    checks but before authentication.
//!
//! 5. **Authentication** - Validates credentials and establishes identity
//!    for the innermost route handlers.
//!
//! # Example
//!
//! ```rust
//! use axum::Router;
//! use nvisy_server::middleware::{
//!     RecoveryConfig, RouterRecoveryExt, RouterObservabilityExt,
//!     RouterSecurityExt, RouterAuthExt,
//! };
//! use nvisy_server::service::ServiceState;
//!
//! fn create_router(state: ServiceState) -> Router {
//!     Router::new()
//!         .with_authentication(state.clone())  // 5. Auth
//!         .with_metrics()                      // 4. Metrics
//!         .with_default_security()             // 3. Security
//!         .with_observability()                // 2. Observability
//!         .with_default_recovery()             // 1. Recovery (outermost)
//! }
//! ```

mod authentication;
mod authorization;
mod constants;
mod observability;
mod recovery;
mod route_category;
mod security;
mod specification;

pub use authentication::{RouterAuthExt, require_authentication, validate_token_middleware};
pub use authorization::require_admin;
pub use constants::{DEFAULT_MAX_BODY_SIZE, DEFAULT_MAX_FILE_BODY_SIZE};
pub use observability::RouterObservabilityExt;
pub use recovery::{RecoveryConfig, RouterRecoveryExt};
pub use route_category::RouteCategory;
pub use security::{
    CorsConfig, FrameOptions, ReferrerPolicy, RouterSecurityExt, SecurityHeadersConfig,
};
pub use specification::{OpenApiConfig, RouterOpenApiExt};
