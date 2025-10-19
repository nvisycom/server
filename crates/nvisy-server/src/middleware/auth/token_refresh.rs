use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use nvisy_postgres::PgDatabase;
use nvisy_postgres::queries::AccountRepository;

use crate::extract::{AuthClaims, AuthHeader, AuthState};
use crate::handler::{ErrorKind, Result};
use crate::service::{AuthKeys, RegionalPolicy};

/// Refreshes the session token if it's close to expiration.
///
/// This middleware will:
/// - Check if the token is expired and return an error if so
/// - Refresh the token if it's within the refresh window (5 minutes before expiration)
/// - Return the response with a new token in the Authorization header if refreshed
pub async fn refresh_token_middleware(
    AuthState(auth_claims): AuthState,
    State(pg_database): State<PgDatabase>,
    State(auth_secret_keys): State<AuthKeys>,
    State(regional_policy): State<RegionalPolicy>,
    request: Request,
    next: Next,
) -> Result<Response> {
    // If token is expired, return unauthorized error
    if auth_claims.is_expired() {
        tracing::warn!(
            target: "server::middleware::auth",
            account_id = auth_claims.account_id.to_string(),
            token_id = auth_claims.token_id.to_string(),
            "Expired token used in request"
        );
        return Err(ErrorKind::Unauthorized.with_context("Authentication token has expired"));
    }

    let mut response = next.run(request).await;

    // If token should be refreshed, refresh it and add to response
    if auth_claims.expires_soon() {
        match refresh_token(&auth_claims, pg_database, auth_secret_keys, regional_policy).await {
            Ok(new_auth_header) => {
                tracing::info!(
                    target: "server::middleware::auth",
                    account_id = auth_claims.account_id.to_string(),
                    old_token_id = auth_claims.token_id.to_string(),
                    new_token_id = new_auth_header.as_auth_claims().token_id.to_string(),
                    "Token refreshed successfully"
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
                    target: "server::middleware::auth",
                    account_id = auth_claims.account_id.to_string(),
                    token_id = auth_claims.token_id.to_string(),
                    error = err.to_string(),
                    "Failed to refresh token"
                );
            }
        }
    }

    Ok(response)
}

/// Helper function to refresh an authentication token
async fn refresh_token(
    auth_claims: &AuthClaims,
    pg_database: PgDatabase,
    auth_secret_keys: AuthKeys,
    regional_policy: RegionalPolicy,
) -> Result<AuthHeader> {
    let mut conn = pg_database.get_connection().await?;

    // First get the current session to obtain the refresh token
    let current_session =
        AccountRepository::find_session_by_access_token(&mut conn, auth_claims.token_id)
            .await?
            .ok_or_else(|| ErrorKind::Unauthorized.into_error())?;

    // Refresh the session using the refresh token
    let updated_session =
        AccountRepository::refresh_session(&mut conn, current_session.refresh_seq).await?;

    // Get the account information
    let account = AccountRepository::find_account_by_id(&mut conn, auth_claims.account_id)
        .await?
        .ok_or_else(|| ErrorKind::Unauthorized.into_error())?;

    // Create new auth claims with updated expiration
    let new_auth_claims = AuthClaims::new(account, updated_session, regional_policy);
    let new_auth_header = AuthHeader::new(new_auth_claims, auth_secret_keys);

    Ok(new_auth_header)
}
