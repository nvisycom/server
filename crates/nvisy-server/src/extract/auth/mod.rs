//! Authentication and authorization module.
//!
//! This module provides comprehensive authentication and authorization functionality
//! for the nvisy API, including JWT token handling, session validation, and
//! permission checking at various levels.
//!
//! # Key Types
//!
//! - [`AuthHeader`] - JWT token extractor and response generator
//! - [`AuthClaims`] - JWT claims structure
//! - [`AuthState`] - Authenticated user state with database verification
//! - [`AuthResult`] - Result of authorization checks
//! - [`Permission`] - Standard project permission levels
//! - [`AuthProvider`] - Trait for types that can perform authorization

// Module declarations
mod auth_provider;
mod auth_state;
mod jwt_header;
mod permission;

pub use self::auth_provider::AuthProvider;
pub use self::auth_state::AuthState;
pub use self::jwt_header::{AuthClaims, AuthHeader};
pub use self::permission::{AuthResult, Permission};

impl AuthProvider for AuthClaims {
    fn account_id(&self) -> uuid::Uuid {
        self.account_id
    }

    fn is_admin(&self) -> bool {
        self.is_administrator
    }
}
