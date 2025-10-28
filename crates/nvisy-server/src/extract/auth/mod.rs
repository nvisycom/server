//! Authentication and authorization module.
//!
//! This module provides comprehensive authentication and authorization functionality
//! for the nvisy API, including JWT token handling, session validation, and
//! permission checking at various levels.
//!
//! # Modules
//!
//! - [`permissions`] - Core authorization types and utilities
//! - [`jwt_header`] - JWT authentication header extraction and generation
//! - [`auth_state`] - Authentication state with database verification
//! - [`auth_provider`] - Authorization provider trait and implementations
//!
//! # Key Types
//!
//! - [`AuthHeader`] - JWT token extractor and response generator
//! - [`AuthClaims`] - JWT claims structure
//! - [`AuthState`] - Authenticated user state with database verification
//! - [`AuthContext`] - Authorization context with user information
//! - [`AuthResult`] - Result of authorization checks
//! - [`ProjectPermission`] - Standard project permission levels
//! - [`AuthProvider`] - Trait for types that can perform authorization

// Module declarations
pub mod auth_provider;
mod auth_state;
mod jwt_header;
mod permission;

pub use self::auth_provider::AuthProvider;
pub use self::auth_state::AuthState;
pub use self::jwt_header::{AuthClaims, AuthHeader};
pub use self::permission::{AuthContext, AuthResult, Permission};

impl AuthProvider for AuthClaims {
    fn account_id(&self) -> uuid::Uuid {
        self.account_id
    }

    fn is_admin(&self) -> bool {
        self.is_administrator
    }
}

impl From<&AuthClaims> for AuthContext {
    fn from(auth_claims: &AuthClaims) -> Self {
        Self {
            account_id: auth_claims.account_id,
            is_admin: auth_claims.is_administrator,
        }
    }
}
