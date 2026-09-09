//! Authentication and authorization module.
//!
//! This module provides comprehensive authentication and authorization functionality
//! for the nvisy API, including JWT token handling, session validation, and
//! permission checking at various levels.

mod auth_state;
mod authorized;
mod jwt_claims;
mod optional_auth;
mod permission;
mod session_token;

pub use self::auth_state::AuthState;
pub use self::authorized::*;
pub use self::jwt_claims::AuthClaims;
pub use self::optional_auth::OptionalAuth;
pub use self::permission::Permission;
pub use self::session_token::{AuthTransport, SessionToken};

/// Name of the `HttpOnly` cookie that carries the session JWT for browser
/// clients. The same JWT reaches programmatic callers as an `Authorization:
/// Bearer` token instead.
pub const SESSION_COOKIE_NAME: &str = "nvisy.session";

/// Name of the readable (non-`HttpOnly`) cookie holding the CSRF token for the
/// double-submit check. It is deliberately readable by client script so the SPA
/// can echo it back in the [`CSRF_HEADER_NAME`] header.
pub const CSRF_COOKIE_NAME: &str = "nvisy.csrf";

/// Request header a cookie-authenticated client must echo the CSRF token in, for
/// the double-submit check on state-changing requests.
pub const CSRF_HEADER_NAME: &str = "x-csrf-token";
