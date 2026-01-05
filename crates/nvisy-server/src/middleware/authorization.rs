//! Authorization middleware for enforcing access control.
//!
//! This module provides middleware for verifying that authenticated users
//! have the required permissions to access specific resources or routes.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::extract::AuthState;
use crate::handler::ErrorKind;
use crate::utility::tracing_targets;

/// Requires the authenticated account to have administrator privileges.
///
/// This middleware must be used after authentication middleware as it
/// depends on the authentication state being present.
pub async fn require_admin(
    AuthState(auth_claims): AuthState,
    request: Request,
    next: Next,
) -> Response {
    if !auth_claims.is_admin {
        tracing::warn!(
            target: tracing_targets::AUTHORIZATION,
            account_id = %auth_claims.account_id,
            "unauthorized admin access attempt"
        );
        return ErrorKind::Unauthorized
            .with_context("Route requires administrator privileges")
            .with_resource("authorization")
            .into_response();
    }

    next.run(request).await
}
