//! Authentication middleware for validating request credentials.
//!
//! This module provides middleware for verifying that requests contain valid
//! authentication tokens.

use axum::Router;
use axum::extract::{Request, State};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::Response;
use nvisy_postgres::PgClient;
use nvisy_postgres::query::AccountApiTokenRepository;

use crate::extract::AuthState;
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;
use crate::utility::tracing_targets;

/// Extension trait for `axum::`[`Router`] to apply authentication middleware.
///
/// This trait provides convenient methods to add authentication requirements
/// to your Axum router, ensuring all routes require valid credentials.
pub trait RouterAuthExt<S> {
    /// Requires valid authentication for all routes.
    ///
    /// This middleware validates the `Authorization` header and ensures
    /// the request has a valid JWT token before proceeding.
    fn with_authentication(self, state: ServiceState) -> Self;

    /// Requires administrator privileges for all routes.
    ///
    /// This middleware must be used after authentication as it depends on
    /// the authentication state being present. Non-admin users receive
    /// a 401 Unauthorized response.
    fn with_admin_authentication(self, state: ServiceState) -> Self;
}

impl<S> RouterAuthExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_authentication(self, state: ServiceState) -> Self {
        self.layer(from_fn_with_state(state, require_authentication))
    }

    fn with_admin_authentication(self, state: ServiceState) -> Self {
        self.layer(from_fn_with_state(state.clone(), super::require_admin))
            .layer(from_fn_with_state(state, require_authentication))
    }
}

/// Requires a valid authentication token to proceed with the request.
///
/// This middleware extracts and validates the `Authorization` header,
/// ensuring the request has a valid JWT token before proceeding.
pub async fn require_authentication(
    AuthState(_): AuthState,
    request: Request,
    next: Next,
) -> Response {
    next.run(request).await
}

/// Validates that the token exists and is not expired.
///
/// This middleware checks the token against the database and returns
/// a 401 Unauthorized response if the token is invalid or expired.
pub async fn validate_token_middleware(
    AuthState(auth_claims): AuthState,
    State(pg_database): State<PgClient>,
    request: Request,
    next: Next,
) -> Result<Response> {
    if auth_claims.is_expired() {
        tracing::warn!(
            target: tracing_targets::AUTHENTICATION,
            account_id = %auth_claims.account_id,
            token_id = %auth_claims.token_id,
            "expired token used in request"
        );
        return Err(ErrorKind::Unauthorized
            .with_context("Authentication token has expired")
            .with_resource("authorization"));
    }

    // Verify token exists in database and update last_used_at
    let mut conn = pg_database.get_connection().await?;
    let token = conn.touch_account_api_token(auth_claims.token_id).await;

    if token.is_err() {
        tracing::warn!(
            target: tracing_targets::AUTHENTICATION,
            account_id = %auth_claims.account_id,
            token_id = %auth_claims.token_id,
            "token not found in database"
        );
        return Err(ErrorKind::Unauthorized
            .with_context("Authentication token not found")
            .with_resource("authorization"));
    }

    Ok(next.run(request).await)
}
