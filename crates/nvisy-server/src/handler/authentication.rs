//! Authentication handlers for user login and registration.
//!
//! This module provides secure authentication endpoints including user login,
//! registration (signup), and logout functionality. All authentication operations
//! follow security best practices including:
//!
//! - Password strength validation using zxcvbn
//! - Secure password hashing with Argon2id
//! - JWT-based session management with EdDSA signatures
//! - Protection against timing attacks
//! - Comprehensive audit logging
//! - Rate limiting and abuse prevention
//!
//! # Security Features
//!
//! ## Password Security
//! - Minimum password strength requirements enforced
//! - Personal information detection (prevents passwords containing name/email)
//! - Secure Argon2id hashing with OWASP recommended parameters
//!
//! ## Session Management
//! - JWT tokens with EdDSA (Ed25519) signatures for maximum security
//! - Configurable token expiration
//! - Automatic token refresh middleware support
//! - Regional data collection policy compliance
//!
//! ## Attack Prevention
//! - Timing attack protection in login verification
//! - Failed login attempt tracking and rate limiting
//! - User agent and IP address logging for forensics
//! - Input validation and sanitization
//!
//! # Endpoints
//!
//! - `POST /auth/login` - Authenticate existing user
//! - `POST /auth/signup` - Register new user account
//! - `POST /auth/logout` - Invalidate current session
//!
//! # Examples
//!
//! ## Login Request
//! ```json
//! {
//!   "emailAddress": "user@example.com",
//!   "password": "secure_password123"
//! }
//! ```
//!
//! ## Signup Request
//! ```json
//! {
//!   "displayName": "John Doe",
//!   "emailAddress": "john@example.com",
//!   "password": "MySecureP@ssw0rd!"
//! }
//! ```
//!
//! # Error Handling
//!
//! All endpoints return standardized error responses with appropriate HTTP status codes:
//! - `400 Bad Request` - Invalid input data or weak passwords
//! - `401 Unauthorized` - Invalid credentials
//! - `409 Conflict` - Email address already registered
//! - `429 Too Many Requests` - Rate limit exceeded
//! - `500 Internal Server Error` - System errors

use axum::extract::State;
use axum::http::StatusCode;
use axum_client_ip::ClientIp;
use axum_extra::TypedHeader;
use axum_extra::headers::UserAgent;
use nvisy_postgres::PgClient;
use nvisy_postgres::models::{Account, AccountApiToken, NewAccount, NewAccountApiToken};
use nvisy_postgres::queries::{AccountApiTokenRepository, AccountRepository};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;
use validator::Validate;

use crate::extract::{AuthClaims, AuthHeader, AuthState, Json, ValidateJson};
use crate::handler::{ErrorKind, ErrorResponse, Result};
use crate::service::{AuthHasher, AuthKeys, PasswordStrength, RegionalPolicy, ServiceState};

/// Tracing target for authentication operations.
const TRACING_TARGET: &str = "nvisy::handler::authentication";

/// Tracing target for authentication cleanup operations.
const TRACING_TARGET_CLEANUP: &str = "nvisy::handler::authentication::cleanup";

/// Creates a new authentication header.
#[tracing::instrument(skip_all)]
fn create_auth_header(
    regional_policy: RegionalPolicy,
    auth_secret_keys: AuthKeys,
    account: Account,
    account_api_token: AccountApiToken,
) -> Result<AuthHeader> {
    let auth_claims = AuthClaims::new(account, account_api_token, regional_policy);
    let auth_header = AuthHeader::new(auth_claims, auth_secret_keys);
    Ok(auth_header)
}

/// Request payload for login.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    #[validate(email)]
    pub email_address: String,
    pub password: String,
}

/// Response returned after successful login.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    pub account_id: Uuid,
    pub token_id: Uuid,
    pub expires_at: time::OffsetDateTime,
    pub regional_policy: String,
}

/// Creates a new account API token.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/auth/login/", tag = "accounts",
    request_body(
        content = LoginRequest,
        description = "Login credentials",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = CREATED,
            description = "API token created",
            body = LoginResponse,
        ),
    ),
)]
async fn login(
    State(pg_database): State<PgClient>,
    State(auth_hasher): State<AuthHasher>,
    State(regional_policy): State<RegionalPolicy>,
    State(auth_keys): State<AuthKeys>,
    ClientIp(ip_address): ClientIp,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<LoginRequest>,
) -> Result<(StatusCode, AuthHeader, Json<LoginResponse>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        email = %request.email_address,
        ip_address = %ip_address,
        "login attempt"
    );

    let mut conn = pg_database.get_connection().await?;
    let normalized_email = request.email_address.to_lowercase();

    let account = AccountRepository::find_account_by_email(&mut conn, &normalized_email).await?;

    // Always perform password hashing to prevent timing attacks
    let password_valid = match &account {
        Some(acc) => auth_hasher
            .verify_password(&request.password, &acc.password_hash)
            .is_ok(),
        None => {
            // Perform dummy hash verification to maintain consistent timing
            // Generate a random password hash to verify against
            use rand::Rng;
            let dummy_password: String = (0..16)
                .map(|_| {
                    let chars = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                    chars[rand::rng().random_range(0..chars.len())] as char
                })
                .collect();

            // Hash the dummy password and verify - this will always fail
            // but takes the same time as a real verification
            if let Ok(dummy_hash) = auth_hasher.hash_password(&dummy_password) {
                let _ = auth_hasher.verify_password(&request.password, &dummy_hash);
            }
            false
        }
    };

    // Check if login should succeed
    let login_successful = matches!(&account, Some(acc) if password_valid && acc.can_login());

    if !login_successful {
        // Record failed login attempt for existing accounts
        if let Some(ref acc) = account
            && let Err(e) = AccountRepository::record_failed_login(&mut conn, acc.id).await
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
    if let Err(e) =
        AccountRepository::record_successful_login(&mut conn, account.id, ip_address.into()).await
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
        region_code: regional_policy.to_string(),
        ..Default::default()
    };
    let account_api_token = AccountApiTokenRepository::create_token(&mut conn, new_token).await?;

    let auth_header = create_auth_header(regional_policy, auth_keys, account, account_api_token)?;

    let auth_claims = auth_header.as_auth_claims();
    let response = LoginResponse {
        account_id: auth_claims.account_id,
        token_id: auth_claims.token_id,
        expires_at: auth_claims.expires_at(),
        regional_policy: regional_policy.to_string(),
    };

    tracing::info!(
        target: TRACING_TARGET,
        token_id = auth_claims.token_id.to_string(),
        account_id = auth_claims.account_id.to_string(),
        email = %normalized_email,
        regional_policy = regional_policy.to_string(),
        "login successful: API token created"
    );

    Ok((StatusCode::CREATED, auth_header, Json(response)))
}

/// Request payload for signup.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
struct SignupRequest {
    #[validate(length(min = 2, max = 32))]
    pub display_name: String,
    #[validate(email)]
    pub email_address: String,
    pub password: String,
}

/// Response returned after successful signup.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct SignupResponse {
    pub account_id: Uuid,
    pub token_id: Uuid,
    pub expires_at: time::OffsetDateTime,
    pub regional_policy: String,
    pub display_name: String,
    pub email_address: String,
}

/// Creates a new account and API token.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/auth/signup/", tag = "accounts",
    request_body(
        content = SignupRequest,
        description = "Signup credentials",
        content_type = "application/json",
    ),
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = CREATED,
            description = "Account created",
            body = SignupResponse,
        ),
    ),
)]
#[allow(clippy::too_many_arguments)]
async fn signup(
    State(pg_database): State<PgClient>,
    State(auth_hasher): State<AuthHasher>,
    State(password_strength): State<PasswordStrength>,
    State(regional_policy): State<RegionalPolicy>,
    State(auth_keys): State<AuthKeys>,
    ClientIp(ip_address): ClientIp,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<SignupRequest>,
) -> Result<(StatusCode, AuthHeader, Json<SignupResponse>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        email = %request.email_address,
        display_name = %request.display_name,
        ip_address = %ip_address,
        "signup attempt"
    );

    let mut conn = pg_database.get_connection().await?;
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
    if AccountRepository::email_exists(&mut conn, &normalized_email).await? {
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

    let account = AccountRepository::create_account(&mut conn, new_account).await?;
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
        region_code: regional_policy.to_string(),
        ..Default::default()
    };
    let account_api_token = AccountApiTokenRepository::create_token(&mut conn, new_token).await?;

    // Extract values before moving account
    let display_name = account.display_name.clone();
    let email_address = account.email_address.clone();

    let auth_header = create_auth_header(regional_policy, auth_keys, account, account_api_token)?;

    let auth_claims = auth_header.as_auth_claims();
    let response = SignupResponse {
        account_id: auth_claims.account_id,
        token_id: auth_claims.token_id,
        expires_at: auth_claims.expires_at(),
        regional_policy: regional_policy.to_string(),
        display_name,
        email_address,
    };

    tracing::info!(
        target: TRACING_TARGET,
        token_id = auth_claims.token_id.to_string(),
        account_id = auth_claims.account_id.to_string(),
        regional_policy = regional_policy.to_string(),
        "signup successful: API token created"
    );

    Ok((StatusCode::CREATED, auth_header, Json(response)))
}

/// Deletes an API token by its ID (from the Authorization header).
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/auth/logout/", tag = "accounts",
    responses(
        (
            status = BAD_REQUEST,
            description = "Bad request",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "API token deleted",
        ),
    ),
)]
async fn logout(
    State(pg_database): State<PgClient>,
    State(regional_policy): State<RegionalPolicy>,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    tracing::trace!(
        target: TRACING_TARGET,
        token_id = auth_claims.token_id.to_string(),
        account_id = auth_claims.account_id.to_string(),
        "logout attempt"
    );

    let mut conn = pg_database.get_connection().await?;

    // Verify API token exists before attempting to delete
    let token_exists =
        AccountApiTokenRepository::find_token_by_access_token(&mut conn, auth_claims.token_id)
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
    let deleted = AccountApiTokenRepository::delete_token(&mut conn, auth_claims.token_id).await?;

    if deleted {
        tracing::info!(
            target: TRACING_TARGET,
            token_id = auth_claims.token_id.to_string(),
            account_id = auth_claims.account_id.to_string(),
            regional_policy = regional_policy.to_string(),
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
        if let Ok(mut cleanup_conn) = pg_database.get_connection().await
            && let Err(e) =
                AccountApiTokenRepository::cleanup_expired_tokens(&mut cleanup_conn).await
        {
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
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new()
        .routes(routes!(login))
        .routes(routes!(signup))
        .routes(routes!(logout))
}

#[cfg(test)]
mod test {
    use crate::handler::projects::routes;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn handlers() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_| routes()).await?;

        // Creates a new account.
        let response = server.post("/auth/signup/").await;
        response.assert_status_success();

        // Logs in to the account.
        let response = server.post("/auth/login/").await;
        response.assert_status_success();

        // Logs out of the account.
        let response = server.post("/auth/logout/").await;
        response.assert_status_success();

        Ok(())
    }
}
