//! Authentication middleware for validating request credentials.
//!
//! This module provides middleware for verifying that requests contain valid
//! authentication tokens and for automatically refreshing tokens near expiration.

use axum::Router;
use axum::extract::{Request, State};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::{AccountApiTokenRepository, AccountRepository};

use crate::extract::{AuthClaims, AuthHeader, AuthState};
use crate::handler::{ErrorKind, Result};
use crate::service::{ServiceState, SessionKeys};
use crate::utility::tracing_targets;

/// Extension trait for `axum::`[`Router`] to apply authentication middleware.
///
/// This trait provides convenient methods to add authentication requirements
/// to your Axum router, ensuring all routes require valid credentials.
pub trait RouterAuthExt<S> {
    /// Requires valid authentication for all routes.
    ///
    /// This middleware validates the `Authorization` header and ensures
    /// the request has a valid JWT token before proceeding. Expired tokens
    /// are automatically refreshed when possible.
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

/// Refreshes the session token if it's close to expiration.
///
/// This middleware automatically refreshes tokens that are within 5 minutes
/// of expiration, providing seamless session continuation for active users.
/// If the token is expired, it returns a 401 Unauthorized response.
pub async fn refresh_token_middleware(
    AuthState(auth_claims): AuthState,
    State(pg_database): State<PgClient>,
    State(auth_secret_keys): State<SessionKeys>,
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

    let mut response = next.run(request).await;

    if auth_claims.expires_soon() {
        match refresh_token(&auth_claims, pg_database, auth_secret_keys).await {
            Ok(new_auth_header) => {
                tracing::info!(
                    target: tracing_targets::AUTHENTICATION,
                    account_id = %auth_claims.account_id,
                    old_token_id = %auth_claims.token_id,
                    new_token_id = %new_auth_header.as_auth_claims().token_id,
                    "token refreshed successfully"
                );

                let new_response = new_auth_header.into_response();
                let (new_parts, _) = new_response.into_parts();

                if let Some(auth_header) = new_parts.headers.get(axum::http::header::AUTHORIZATION)
                {
                    let (mut parts, body) = response.into_parts();
                    parts
                        .headers
                        .insert(axum::http::header::AUTHORIZATION, auth_header.clone());
                    response = Response::from_parts(parts, body);
                }
            }
            Err(err) => {
                tracing::error!(
                    target: tracing_targets::AUTHENTICATION,
                    account_id = %auth_claims.account_id,
                    token_id = %auth_claims.token_id,
                    error = %err,
                    "failed to refresh token"
                );
            }
        }
    }

    Ok(response)
}

async fn refresh_token(
    auth_claims: &AuthClaims,
    pg_database: PgClient,
    auth_secret_keys: SessionKeys,
) -> Result<AuthHeader> {
    let mut conn = pg_database.get_connection().await?;

    let current_token = conn
        .find_token_by_access_token(auth_claims.token_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::Unauthorized
                .with_message("Account API token not found")
                .with_context(format!("Account API token ID: {}", auth_claims.token_id))
                .with_resource("authentication")
        })?;

    let updated_token = conn.refresh_token(current_token.refresh_seq).await?;

    let account = conn
        .find_account_by_id(auth_claims.account_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::Unauthorized
                .with_message("Account not found")
                .with_context(format!("Account ID: {}", auth_claims.account_id))
                .with_resource("authentication")
        })?;

    let new_auth_claims = AuthClaims::new(&account, &updated_token);
    let new_auth_header = AuthHeader::new(new_auth_claims, auth_secret_keys);
    Ok(new_auth_header)
}
