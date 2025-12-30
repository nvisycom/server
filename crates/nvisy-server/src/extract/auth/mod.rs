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
pub use crate::utility::tracing_targets::{
    AUTHENTICATION as TRACING_TARGET_AUTHENTICATION, AUTHORIZATION as TRACING_TARGET_AUTHORIZATION,
};

impl<T> AuthProvider for AuthClaims<T> {
    fn account_id(&self) -> uuid::Uuid {
        self.account_id
    }

    fn is_admin(&self) -> bool {
        self.is_owner
    }
}
