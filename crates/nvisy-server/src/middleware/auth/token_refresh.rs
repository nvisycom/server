use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use nvisy_postgres::PgClient;
use nvisy_postgres::query::{AccountApiTokenRepository, AccountRepository};

use crate::extract::{AuthClaims, AuthHeader, AuthState};
use crate::handler::{ErrorKind, Result};
use crate::middleware::TRACING_TARGET_AUTH;
use crate::service::SessionKeys;

/// Refreshes the session token if it's close to expiration.
///
/// This middleware will:
///
/// - Check if the token is expired and return an error if so
/// - Refresh the token if it's within the refresh window (5 minutes before expiration)
/// - Return the response with a new token in the Authorization header if refreshed
pub async fn refresh_token_middleware(
    AuthState(auth_claims): AuthState,
    State(pg_database): State<PgClient>,
    State(auth_secret_keys): State<SessionKeys>,
    request: Request,
    next: Next,
) -> Result<Response> {
    // If token is expired, return unauthorized error
    if auth_claims.is_expired() {
        tracing::warn!(
            target: TRACING_TARGET_AUTH,
            account_id = auth_claims.account_id.to_string(),
            token_id = auth_claims.token_id.to_string(),
            "expired token used in request"
        );
        return Err(ErrorKind::Unauthorized
            .with_context("Authentication token has expired")
            .with_resource("authorization"));
    }

    let mut response = next.run(request).await;

    // If token should be refreshed, refresh it and add to response
    if auth_claims.expires_soon() {
        match refresh_token(&auth_claims, pg_database, auth_secret_keys).await {
            Ok(new_auth_header) => {
                tracing::info!(
                    target: TRACING_TARGET_AUTH,
                    account_id = auth_claims.account_id.to_string(),
                    old_token_id = auth_claims.token_id.to_string(),
                    new_token_id = new_auth_header.as_auth_claims().token_id.to_string(),
                    "token refreshed successfully"
                );

                // Add the new token to the response by converting it to a full response
                // and extracting the Authorization header
                let new_response = new_auth_header.into_response();
                let (new_parts, _) = new_response.into_parts();

                // Extract the Authorization header from the new response
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
                    target: TRACING_TARGET_AUTH,
                    account_id = auth_claims.account_id.to_string(),
                    token_id = auth_claims.token_id.to_string(),
                    error = err.to_string(),
                    "failed to refresh token"
                );
            }
        }
    }

    Ok(response)
}

/// Helper function to refresh an authentication token
async fn refresh_token(
    auth_claims: &AuthClaims,
    pg_database: PgClient,
    auth_secret_keys: SessionKeys,
) -> Result<AuthHeader> {
    let mut conn = pg_database.get_connection().await?;

    // First get the current API token to obtain the refresh token
    let current_token =
        AccountApiTokenRepository::find_token_by_access_token(&mut conn, auth_claims.token_id)
            .await?
            .ok_or_else(|| {
                ErrorKind::Unauthorized
                    .with_message("Account API token not found")
                    .with_context(format!("Account API token ID: {}", auth_claims.token_id))
                    .with_resource("authentication")
            })?;

    // Refresh the API token using the refresh token
    let updated_token =
        AccountApiTokenRepository::refresh_token(&mut conn, current_token.refresh_seq).await?;

    // Get the account information
    let account = AccountRepository::find_account_by_id(&mut conn, auth_claims.account_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::Unauthorized
                .with_message("Account not found")
                .with_context(format!("Account ID: {}", auth_claims.account_id))
                .with_resource("authentication")
        })?;

    // Create new auth claims with updated expiration
    let new_auth_claims = AuthClaims::new(account, updated_token);
    let new_auth_header = AuthHeader::new(new_auth_claims, auth_secret_keys);

    Ok(new_auth_header)
}
