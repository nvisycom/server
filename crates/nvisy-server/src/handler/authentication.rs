//! Authentication handlers for user login and registration.
//!
//! This module provides secure authentication endpoints including user login,
//! registration (signup), and logout functionality. All authentication operations
//! follow security best practices including:

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use axum_extra::headers::UserAgent;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{Account, AccountApiToken, NewAccount, NewAccountApiToken};
use nvisy_postgres::query::{AccountApiTokenRepository, AccountRepository};
use nvisy_postgres::types::ApiTokenType;

use super::request::{Login, Signup};
use super::response::AuthToken;
use crate::extract::{
    AuthClaims, AuthHeader, AuthState, ClientIp, Json, TypedHeader, ValidateJson,
};
use crate::handler::{ErrorKind, Result};
use crate::service::{PasswordHasher, PasswordStrength, ServiceState, SessionKeys};

/// Tracing target for authentication operations.
const TRACING_TARGET: &str = "nvisy_server::handler::authentication";

/// Tracing target for authentication cleanup operations.
const TRACING_TARGET_CLEANUP: &str = "nvisy_server::handler::authentication::cleanup";

/// Creates a new authentication header.
#[tracing::instrument(skip_all)]
fn create_auth_header(
    auth_secret_keys: SessionKeys,
    account_model: &Account,
    account_api_token: &AccountApiToken,
) -> Result<AuthHeader> {
    let auth_claims = AuthClaims::new(account_model, account_api_token);
    let auth_header = AuthHeader::new(auth_claims, auth_secret_keys);
    Ok(auth_header)
}

/// Creates a new account API token.
#[tracing::instrument(skip_all)]
async fn login(
    State(pg_client): State<PgClient>,
    State(auth_hasher): State<PasswordHasher>,
    State(auth_keys): State<SessionKeys>,
    ClientIp(ip_address): ClientIp,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<Login>,
) -> Result<(StatusCode, AuthHeader, Json<AuthToken>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        email = %request.email_address,
        ip_address = %ip_address,
        "login attempt"
    );

    let normalized_email = request.email_address.to_lowercase();
    let account = pg_client.find_account_by_email(&normalized_email).await?;

    // Always perform password hashing to prevent timing attacks
    let password_valid = match &account {
        Some(acc) => auth_hasher
            .verify_password(&request.password, &acc.password_hash)
            .is_ok(),
        None => {
            // Perform dummy hash verification to maintain consistent timing
            // and prevent account enumeration via timing attacks
            auth_hasher.verify_dummy_password(&request.password)
        }
    };

    // Check if login should succeed
    let login_successful = matches!(&account, Some(acc) if password_valid && acc.can_login());

    if !login_successful {
        // Record failed login attempt for existing accounts
        if let Some(ref acc) = account
            && let Err(e) = pg_client.record_failed_login(acc.id).await
        {
            tracing::error!(
                target: TRACING_TARGET,
                account_id = acc.id.to_string(),
                error = %e,
                "failed to record failed login attempt"
            );
        }

        tracing::warn!(
            target: TRACING_TARGET,
            email = %normalized_email,
            account_exists = account.is_some(),
            password_valid = password_valid,
            "login failed"
        );

        return Err(ErrorKind::NotFound.into_error());
    }

    let account = account.unwrap(); // Safe because we verified above

    // Record successful login
    if let Err(e) = pg_client
        .record_successful_login(account.id, ip_address.into())
        .await
    {
        tracing::error!(
            target: TRACING_TARGET,
            account_id = account.id.to_string(),
            error = %e,
            "failed to record successful login"
        );
    }

    let new_token = NewAccountApiToken {
        account_id: account.id,
        ip_address: ip_address.into(),
        user_agent: user_agent.to_string(),
        is_remembered: Some(request.remember_me),
        session_type: Some(ApiTokenType::Web),
        ..Default::default()
    };

    let account_api_token = pg_client.create_token(new_token).await?;
    let auth_header = create_auth_header(auth_keys, &account, &account_api_token)?;

    let auth_claims = auth_header.as_auth_claims();
    let response = AuthToken {
        account_id: auth_claims.account_id,
        display_name: account.display_name.clone(),
        email_address: account.email_address.clone(),
        issued_at: auth_claims.issued_at,
        expires_at: auth_claims.expires_at,
    };

    tracing::info!(
        target: TRACING_TARGET,
        token_id = auth_claims.token_id.to_string(),
        account_id = auth_claims.account_id.to_string(),
        email = %normalized_email,
        "login successful: API token created"
    );

    Ok((StatusCode::CREATED, auth_header, Json(response)))
}

/// Creates a new account and API token.
#[tracing::instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
async fn signup(
    State(pg_client): State<PgClient>,
    State(auth_hasher): State<PasswordHasher>,
    State(password_strength): State<PasswordStrength>,
    State(auth_keys): State<SessionKeys>,
    ClientIp(ip_address): ClientIp,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<Signup>,
) -> Result<(StatusCode, AuthHeader, Json<AuthToken>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        email = %request.email_address,
        display_name = %request.display_name,
        ip_address = %ip_address,
        "signup attempt"
    );

    let normalized_email = request.email_address.to_lowercase();

    // Validate password strength
    let email_parts: Vec<&str> = normalized_email.split('@').collect();
    let mut user_inputs = vec![request.display_name.as_str()];
    user_inputs.extend(email_parts);

    password_strength
        .validate_password(&request.password, &user_inputs)
        .map_err(|_| ErrorKind::BadRequest.into_error())?;

    let password_hash = auth_hasher
        .hash_password(&request.password)
        .map_err(|_| ErrorKind::InternalServerError.into_error())?;

    // Check if email already exists
    if pg_client.email_exists(&normalized_email).await? {
        tracing::warn!(
            target: TRACING_TARGET,
            email = %normalized_email,
            "signup failed: email already exists"
        );
        return Err(ErrorKind::Conflict.into_error());
    }

    let new_account = NewAccount {
        display_name: request.display_name,
        email_address: normalized_email.clone(),
        password_hash,
        ..Default::default()
    };

    let account = pg_client.create_account(new_account).await?;
    tracing::info!(
        target: TRACING_TARGET,
        account_id = account.id.to_string(),
        email = %account.email_address,
        display_name = %account.display_name,
        "account created"
    );

    let new_token = NewAccountApiToken {
        account_id: account.id,
        ip_address: ip_address.into(),
        user_agent: user_agent.to_string(),
        is_remembered: Some(request.remember_me),
        session_type: Some(ApiTokenType::Web),
        ..Default::default()
    };
    let account_api_token = pg_client.create_token(new_token).await?;

    // Extract values before moving account
    let display_name = account.display_name.clone();
    let email_address = account.email_address.clone();

    let auth_header = create_auth_header(auth_keys, &account, &account_api_token)?;

    let auth_claims = auth_header.as_auth_claims();
    let response = AuthToken {
        account_id: auth_claims.account_id,
        display_name,
        email_address,
        issued_at: auth_claims.issued_at,
        expires_at: auth_claims.expires_at,
    };

    tracing::info!(
        target: TRACING_TARGET,
        token_id = auth_claims.token_id.to_string(),
        account_id = auth_claims.account_id.to_string(),
        "signup successful: API token created"
    );

    Ok((StatusCode::CREATED, auth_header, Json(response)))
}

/// Deletes an API token by its ID (from the Authorization header).
#[tracing::instrument(skip_all)]
async fn logout(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    tracing::trace!(
        target: TRACING_TARGET,
        token_id = auth_claims.token_id.to_string(),
        account_id = auth_claims.account_id.to_string(),
        "logout attempt"
    );

    // Verify API token exists before attempting to delete
    let token_exists = pg_client
        .find_token_by_access_token(auth_claims.token_id)
        .await?
        .is_some();

    if !token_exists {
        tracing::warn!(
            target: TRACING_TARGET,
            token_id = auth_claims.token_id.to_string(),
            account_id = auth_claims.account_id.to_string(),
            "logout attempted on non-existent API token"
        );
        return Ok(StatusCode::OK); // Consider it successful if token doesn't exist
    }

    // Delete the API token
    let deleted = pg_client.delete_token(auth_claims.token_id).await?;

    if deleted {
        tracing::info!(
            target: TRACING_TARGET,
            token_id = auth_claims.token_id.to_string(),
            account_id = auth_claims.account_id.to_string(),
            "logout successful: API token deleted"
        );
    } else {
        tracing::warn!(
            target: TRACING_TARGET,
            token_id = auth_claims.token_id.to_string(),
            account_id = auth_claims.account_id.to_string(),
            "logout completed but API token was not found for deletion"
        );
    }

    // Opportunistically clean up expired sessions for this account (fire and forget)
    tokio::spawn(async move {
        if let Err(e) = pg_client.cleanup_expired_tokens().await {
            tracing::debug!(
                target: TRACING_TARGET_CLEANUP,
                error = %e,
                "failed to cleanup expired sessions during logout"
            );
        }
    });

    Ok(StatusCode::OK)
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/auth/login", post(login))
        .api_route("/auth/signup", post(signup))
        .api_route("/auth/logout", post(logout))
}

