//! Authentication handlers for user login and registration.
//!
//! This module provides secure authentication endpoints including user login,
//! registration (signup), and logout functionality. All authentication operations
//! follow security best practices including:

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use axum_extra::headers::UserAgent;
use jiff::{Span, Timestamp};
use nvisy_postgres::JiffTimestamp;
use nvisy_postgres::model::{Account, AccountApiToken, NewAccount, NewAccountApiToken};
use nvisy_postgres::query::{AccountApiTokenRepository, AccountRepository};
use nvisy_postgres::types::{ApiTokenType, HasDeletedAt};

use super::request::{Login, Signup};
use super::response::{AuthToken, ErrorResponse};
use crate::extract::{AuthClaims, AuthHeader, AuthState, Json, PgPool, TypedHeader, ValidateJson};
use crate::handler::{ErrorKind, Result};
use crate::service::{AuthKeys, PasswordHasher, PasswordStrength, ServiceState, UserAgentParser};

/// Tracing target for authentication operations.
const TRACING_TARGET: &str = "nvisy_server::handler::authentication";

/// Tracing target for authentication cleanup operations.
const TRACING_TARGET_CLEANUP: &str = "nvisy_server::handler::authentication::cleanup";

/// Builds user inputs for password strength validation.
fn build_password_user_inputs<'a>(display_name: &'a str, email_address: &'a str) -> Vec<&'a str> {
    let mut inputs = vec![display_name];
    inputs.extend(email_address.split('@'));
    inputs
}

/// Creates a new authentication header.
fn create_auth_header(
    auth_secret_keys: AuthKeys,
    account_model: &Account,
    account_api_token: &AccountApiToken,
) -> Result<AuthHeader> {
    let auth_claims = AuthClaims::new(account_model, account_api_token);
    let auth_header = AuthHeader::new(auth_claims, auth_secret_keys);
    Ok(auth_header)
}

/// Creates a new account API token (login).
#[tracing::instrument(skip_all)]
async fn login(
    PgPool(mut conn): PgPool,
    State(auth_hasher): State<PasswordHasher>,
    State(auth_keys): State<AuthKeys>,
    State(ua_parser): State<UserAgentParser>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<Login>,
) -> Result<(StatusCode, Json<AuthToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Login attempt");

    let account = conn.find_account_by_email(&request.email_address).await?;

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

    // Check for login failures and return appropriate errors
    match &account {
        None => {
            tracing::warn!(target: TRACING_TARGET, reason = "account_not_found", "Login failed");
            return Err(ErrorKind::Unauthorized
                .with_resource("credentials")
                .with_message("Invalid email or password"));
        }
        Some(_) if !password_valid => {
            tracing::warn!(target: TRACING_TARGET, reason = "invalid_password", "Login failed");
            return Err(ErrorKind::Unauthorized
                .with_resource("credentials")
                .with_message("Invalid email or password"));
        }
        Some(acc) if acc.is_suspended() => {
            tracing::warn!(target: TRACING_TARGET, reason = "account_suspended", "Login failed");
            return Err(ErrorKind::Forbidden
                .with_resource("account")
                .with_message("Account is suspended"));
        }
        Some(acc) if acc.is_deleted() => {
            tracing::warn!(target: TRACING_TARGET, reason = "account_deleted", "Login failed");
            return Err(ErrorKind::Forbidden
                .with_resource("account")
                .with_message("Account has been deleted"));
        }
        _ => {}
    }

    let account = account.unwrap(); // Safe because we verified above
    let expired_at = Timestamp::now() + Span::new().hours(90 * 24);
    let new_token = NewAccountApiToken {
        account_id: account.id,
        name: ua_parser.parse(user_agent.as_str()),
        ip_address: crate::utility::placeholder_ip(),
        user_agent: user_agent.to_string(),
        is_remembered: Some(request.remember_me),
        session_type: Some(ApiTokenType::Web),
        expired_at: Some(expired_at.into()),
        ..Default::default()
    };

    let account_api_token = conn.create_token(new_token).await?;
    let auth_header = create_auth_header(auth_keys, &account, &account_api_token)?;

    let auth_claims = auth_header.as_auth_claims();
    let api_token = auth_header.into_string()?;
    let response = AuthToken {
        api_token,
        token_id: auth_claims.token_id,
        account_id: auth_claims.account_id,
        display_name: account.display_name.clone(),
        email_address: account.email_address.clone(),
        issued_at: Timestamp::from_second(auth_claims.issued_at).unwrap_or(Timestamp::now()),
        expires_at: Timestamp::from_second(auth_claims.expires_at).unwrap_or(Timestamp::now()),
    };

    tracing::info!(
        target: TRACING_TARGET,
        token_id = %auth_claims.token_id,
        account_id = %auth_claims.account_id,
        "Login successful",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn login_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Login")
        .description("Authenticates a user and returns an access token.")
        .response::<201, Json<AuthToken>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Creates a new account and API token (signup).
#[tracing::instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
async fn signup(
    PgPool(mut conn): PgPool,
    State(auth_hasher): State<PasswordHasher>,
    State(password_strength): State<PasswordStrength>,
    State(auth_keys): State<AuthKeys>,
    State(ua_parser): State<UserAgentParser>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<Signup>,
) -> Result<(StatusCode, Json<AuthToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Signing up");

    // Validate password strength and hash
    let user_inputs = build_password_user_inputs(&request.display_name, &request.email_address);
    password_strength.validate_password(&request.password, &user_inputs)?;
    let password_hash = auth_hasher.hash_password(&request.password)?;

    // Check if email already exists
    if conn.email_exists(&request.email_address).await? {
        tracing::warn!(target: TRACING_TARGET, "Signup failed: email already exists");
        return Err(ErrorKind::Conflict.into_error());
    }

    let new_account = NewAccount {
        display_name: request.display_name,
        email_address: request.email_address,
        password_hash,
        ..Default::default()
    };

    let account = conn.create_account(new_account).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account.id,
        "Account created",
    );

    let expired_at = Timestamp::now()
        .checked_add(Span::new().hours(90 * 24))
        .ok()
        .map(JiffTimestamp::from);

    let user_agent_str = user_agent.to_string();
    let new_token = NewAccountApiToken {
        account_id: account.id,
        name: ua_parser.parse(&user_agent_str),
        ip_address: crate::utility::placeholder_ip(),
        user_agent: user_agent_str,
        is_remembered: Some(request.remember_me),
        session_type: Some(ApiTokenType::Web),
        expired_at,
        ..Default::default()
    };
    let account_api_token = conn.create_token(new_token).await?;

    // Extract values before moving account
    let display_name = account.display_name.clone();
    let email_address = account.email_address.clone();

    let auth_header = create_auth_header(auth_keys, &account, &account_api_token)?;

    let auth_claims = auth_header.as_auth_claims();
    let api_token = auth_header.into_string()?;
    let response = AuthToken {
        api_token,
        token_id: auth_claims.token_id,
        account_id: auth_claims.account_id,
        display_name,
        email_address,
        issued_at: Timestamp::from_second(auth_claims.issued_at).unwrap_or(Timestamp::now()),
        expires_at: Timestamp::from_second(auth_claims.expires_at).unwrap_or(Timestamp::now()),
    };

    tracing::info!(
        target: TRACING_TARGET,
        token_id = %auth_claims.token_id,
        account_id = %auth_claims.account_id,
        "Signup successful",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn signup_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Signup")
        .description("Creates a new account and returns an access token.")
        .response::<201, Json<AuthToken>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Deletes an API token by its ID (logout).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_claims.account_id,
        token_id = %auth_claims.token_id,
    )
)]
async fn logout(PgPool(mut conn): PgPool, AuthState(auth_claims): AuthState) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Logging out");

    // Verify API token exists before attempting to delete
    let token_exists = conn.find_token_by_id(auth_claims.token_id).await?.is_some();

    if !token_exists {
        tracing::warn!(target: TRACING_TARGET, "Logout attempted on non-existent token");
        return Ok(StatusCode::OK); // Consider it successful if token doesn't exist
    }

    // Delete the API token
    let deleted = conn.delete_token_by_id(auth_claims.token_id).await?;

    if deleted {
        tracing::info!(target: TRACING_TARGET, "Logout successful");
    } else {
        tracing::warn!(target: TRACING_TARGET, "Logout completed but token was not found");
    }

    // Opportunistically clean up expired sessions for this account (fire and forget)
    tokio::spawn(async move {
        if let Err(e) = conn.cleanup_expired_tokens().await {
            tracing::debug!(
                target: TRACING_TARGET_CLEANUP,
                error = %e,
                "Failed to cleanup expired sessions during logout"
            );
        }
    });

    Ok(StatusCode::OK)
}

fn logout_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Logout")
        .description("Invalidates the current access token.")
        .response_with::<200, (), _>(|res| res.description("Logged out."))
        .response::<401, Json<ErrorResponse>>()
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/auth/login", post_with(login, login_docs))
        .api_route("/auth/signup", post_with(signup, signup_docs))
        .api_route("/auth/logout", post_with(logout, logout_docs))
        .with_path_items(|item| item.tag("Authentication"))
}
