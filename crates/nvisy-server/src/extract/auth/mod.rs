//! Authentication and authorization module.
//!
//! This module provides comprehensive authentication and authorization functionality
//! for the nvisy API, including JWT token handling, session validation, and
//! permission checking at various levels.

mod auth_provider;
mod auth_state;
mod jwt_claims;
mod jwt_header;
mod permission;

pub use self::auth_provider::AuthProvider;
pub use self::auth_state::AuthState;
pub use self::jwt_claims::AuthClaims;
pub use self::jwt_header::AuthHeader;
pub use self::permission::{AuthResult, Permission};

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

impl<T> AuthProvider for AuthClaims<T> {
    fn account_id(&self) -> uuid::Uuid {
        self.account_id
    }

    fn is_admin(&self) -> bool {
        self.is_administrator
    }
}
