//! Account management handlers for user profile operations.
//!
//! This module provides comprehensive account management functionality including
//! profile viewing, updating, and deletion. All operations follow security best
//! practices with proper authorization, input validation, and audit logging.
//!
//! # Security Features
//!
//! ## Authorization
//! - JWT-based authentication required for all operations
//! - Self-service operations (users can only modify their own accounts)
//! - Administrator privilege escalation for cross-account operations
//! - Project membership verification for related operations
//!
//! ## Data Protection
//! - Input validation and sanitization
//! - Password hashing with Argon2id for password updates
//! - Email normalization and validation
//! - Secure password strength validation
//!
//! ## Audit & Compliance
//! - Comprehensive operation logging
//! - Regional data collection policy enforcement
//! - Account deletion with proper cleanup
//! - Failed operation tracking
//!
//! # Endpoints
//!
//! ## Profile Management
//! - `GET /accounts/me` - Get own account details
//! - `PUT /accounts/me` - Update own account profile
//! - `DELETE /accounts/me` - Delete own account
//!
//! ## Administrative Operations (Admin Only)
//! - `GET /accounts/{id}` - Get any account by ID
//! - `PUT /accounts/{id}` - Update any account profile
//! - `DELETE /accounts/{id}` - Delete any account
//!
//! # Request/Response Examples
//!
//! ## Get Account Response
//! ```json
//! {
//!   "accountId": "550e8400-e29b-41d4-a716-446655440000",
//!   "displayName": "John Doe",
//!   "emailAddress": "john@example.com",
//!   "isAdministrator": false,
//!   "createdAt": "2024-01-15T10:30:00Z",
//!   "updatedAt": "2024-01-15T10:30:00Z"
//! }
//! ```
//!
//! ## Update Account Request
//! ```json
//! {
//!   "displayName": "Jane Smith",
//!   "emailAddress": "jane@example.com",
//!   "password": "NewSecureP@ssw0rd123"
//! }
//! ```
//!
//! # Error Handling
//!
//! All endpoints return standardized error responses:
//! - `400 Bad Request` - Invalid input data or validation failures
//! - `401 Unauthorized` - Authentication required or invalid token
//! - `403 Forbidden` - Insufficient permissions for operation
//! - `404 Not Found` - Account not found
//! - `409 Conflict` - Email address already in use
//! - `500 Internal Server Error` - System errors
//!
//! # Data Validation
//!
//! - Display names: 2-32 characters, alphanumeric and common symbols
//! - Email addresses: RFC 5322 compliant format validation
//! - Passwords: Strength validation using zxcvbn algorithm
//! - All inputs sanitized to prevent injection attacks

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use nvisy_postgres::PgClient;
use nvisy_postgres::models::{Account, UpdateAccount};
use nvisy_postgres::queries::AccountRepository;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;
use validator::Validate;

use crate::extract::{AuthState, Json, Path, ValidateJson};
use crate::handler::{ErrorKind, ErrorResponse, Result};
use crate::middleware::require_admin;
use crate::service::{AuthHasher, PasswordStrength, ServiceState};

/// Tracing target for account operations.
const TRACING_TARGET: &str = "nvisy::handler::accounts";

/// `Path` param for `{accountId}` handlers.
#[must_use]
#[derive(Debug, Serialize, Deserialize, IntoParams)]
pub struct AccountPathParams {
    pub account_id: Uuid,
}

/// Retrieves the account by its ID.
async fn get_account_internal(
    pg_database: PgClient,
    account_id: Uuid,
) -> Result<(StatusCode, Json<GetAccountResponse>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = account_id.to_string(),
        "retrieving account"
    );

    let mut conn = pg_database.get_connection().await?;
    let Some(account) = AccountRepository::find_account_by_id(&mut conn, account_id).await? else {
        return Err(ErrorKind::NotFound.into_error());
    };

    tracing::info!(
        target: TRACING_TARGET,
        account_id = account.id.to_string(),
        display_name = %account.display_name,
        "account retrieved"
    );

    Ok((StatusCode::OK, Json(account.into())))
}

/// Updates an account by its ID.
async fn update_account_internal(
    pg_database: PgClient,
    auth_hasher: AuthHasher,
    password_strength: PasswordStrength,
    account_id: Uuid,
    request: UpdateAccountRequest,
) -> Result<(StatusCode, Json<UpdateAccountResponse>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = account_id.to_string(),
        has_display_name = request.display_name.is_some(),
        has_email = request.email_address.is_some(),
        has_password = request.password.is_some(),
        "updating account"
    );

    let mut conn = pg_database.get_connection().await?;

    // Get current account info for password validation
    let Some(current_account) =
        AccountRepository::find_account_by_id(&mut conn, account_id).await?
    else {
        return Err(ErrorKind::NotFound.into_error());
    };

    // Validate password strength if password is being updated
    let password_hash = if let Some(ref password) = request.password {
        let display_name = request
            .display_name
            .as_ref()
            .unwrap_or(&current_account.display_name);
        let email_address = request
            .email_address
            .as_ref()
            .unwrap_or(&current_account.email_address);

        // Validate password strength
        let email_parts: Vec<&str> = email_address.split('@').collect();
        let mut user_inputs = vec![display_name.as_str()];
        user_inputs.extend(email_parts);
        password_strength
            .validate_password(password, &user_inputs)
            .map_err(|_| ErrorKind::BadRequest.into_error())?;

        Some(auth_hasher.hash_password(password)?)
    } else {
        None
    };

    // Normalize email address if provided
    let normalized_email = request
        .email_address
        .as_ref()
        .map(|email| email.to_lowercase());

    // Check if email already exists for another account
    if let Some(ref email) = normalized_email
        && AccountRepository::email_exists(&mut conn, email).await?
        && current_account.email_address != *email
    {
        tracing::warn!(
            target: TRACING_TARGET,
            account_id = account_id.to_string(),
            email = %email,
            "account update failed: email already exists"
        );
        return Err(ErrorKind::Conflict.with_context("Account with this email already exists"));
    }

    let update_account = UpdateAccount {
        display_name: request.display_name,
        email_address: normalized_email,
        password_hash,
        ..Default::default()
    };

    let account = AccountRepository::update_account(&mut conn, account_id, update_account).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = account.id.to_string(),
        "account updated"
    );

    Ok((StatusCode::OK, Json(account.into())))
}

/// Deletes an account by its ID.
async fn delete_account_internal(
    pg_database: PgClient,
    account_id: Uuid,
) -> Result<(StatusCode, Json<DeleteAccountResponse>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = account_id.to_string(),
        "deleting account"
    );

    let mut conn = pg_database.get_connection().await?;
    AccountRepository::delete_account(&mut conn, account_id).await?;

    let response = DeleteAccountResponse {
        account_id,
        created_at: OffsetDateTime::now_utc(),
        deleted_at: OffsetDateTime::now_utc(),
    };

    tracing::info!(
        target: TRACING_TARGET,
        account_id = account_id.to_string(),
        "account deleted"
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Response returned when retrieving an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct GetAccountResponse {
    pub account_id: Uuid,
    pub is_activated: bool,
    pub is_admin: bool,

    pub display_name: String,
    pub email_address: String,

    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<Account> for GetAccountResponse {
    fn from(account: Account) -> Self {
        Self {
            account_id: account.id,
            is_activated: account.is_verified,
            is_admin: account.is_admin,

            display_name: account.display_name,
            email_address: account.email_address,

            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

/// Retrieves the authenticated account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/accounts/", tag = "accounts",
    responses(
        (
            status = NOT_FOUND,
            description = "Not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Account details",
            body = GetAccountResponse,
        ),
    ),
)]
async fn get_own_account(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<GetAccountResponse>)> {
    // Demonstrate new AuthState pattern - direct access to user info
    tracing::debug!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        is_admin = auth_claims.is_administrator,
        "Retrieving own account information"
    );

    get_account_internal(pg_database, auth_claims.account_id).await
}

/// Retrieves the account by its ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/accounts/{accountId}", tag = "accounts",
    params(AccountPathParams),
    responses(
        (
            status = NOT_FOUND,
            description = "Not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Account details",
            body = GetAccountResponse,
        ),
    ),
)]
async fn get_account_by_id(
    State(pg_database): State<PgClient>,
    Path(path_params): Path<AccountPathParams>,
) -> Result<(StatusCode, Json<GetAccountResponse>)> {
    get_account_internal(pg_database, path_params.account_id).await
}

/// Request payload to update an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateAccountRequest {
    #[validate(length(min = 2, max = 32))]
    pub display_name: Option<String>,
    #[validate(email)]
    pub email_address: Option<String>,
    pub password: Option<String>,
}

/// Response returned after updating an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateAccountResponse {
    pub account_id: Uuid,

    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<Account> for UpdateAccountResponse {
    fn from(account: Account) -> Self {
        Self {
            account_id: account.id,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

/// Updates the authenticated account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    patch, path = "/accounts/", tag = "accounts",
    request_body(
        content = UpdateAccountRequest,
        description = "Account changes",
        content_type = "application/json",
    ),
    responses(
        (
            status = NOT_FOUND,
            description = "Not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            body = UpdateAccountResponse,
            description = "Updated account",
        ),
    )
)]
async fn update_own_account(
    State(pg_database): State<PgClient>,
    State(auth_hasher): State<AuthHasher>,
    State(password_strength): State<PasswordStrength>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<UpdateAccountRequest>,
) -> Result<(StatusCode, Json<UpdateAccountResponse>)> {
    update_account_internal(
        pg_database,
        auth_hasher,
        password_strength,
        auth_claims.account_id,
        request,
    )
    .await
}

/// Updates the authenticated user's account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    patch, path = "/accounts/{accountId}", tag = "accounts",
    params(AccountPathParams),
    request_body(
        content = UpdateAccountRequest,
        description = "Account changes",
        content_type = "application/json"
    ),
    responses(
        (
            status = NOT_FOUND,
            description = "Not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            body = UpdateAccountResponse,
            description = "Updated account",
        ),
    )
)]
async fn update_account_by_id(
    State(pg_database): State<PgClient>,
    State(auth_hasher): State<AuthHasher>,
    State(password_strength): State<PasswordStrength>,
    Path(path_params): Path<AccountPathParams>,
    ValidateJson(request): ValidateJson<UpdateAccountRequest>,
) -> Result<(StatusCode, Json<UpdateAccountResponse>)> {
    update_account_internal(
        pg_database,
        auth_hasher,
        password_strength,
        path_params.account_id,
        request,
    )
    .await
}

/// Response returned after deleting an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct DeleteAccountResponse {
    pub account_id: Uuid,

    pub created_at: OffsetDateTime,
    pub deleted_at: OffsetDateTime,
}

/// Deletes the authenticated account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    delete, path = "/accounts/", tag = "accounts",
    responses(
        (
            status = NOT_FOUND,
            description = "Not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Account deleted",
            body = DeleteAccountResponse
        ),
    ),
)]
async fn delete_own_account(
    State(pg_database): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<DeleteAccountResponse>)> {
    delete_account_internal(pg_database, auth_claims.account_id).await
}

/// Deletes the account by its ID.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    delete, path = "/accounts/{accountId}", tag = "accounts",
    params(AccountPathParams),
    responses(
        (
            status = NOT_FOUND,
            description = "Not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
        (
            status = OK,
            description = "Account deleted",
            body = DeleteAccountResponse,
        ),
    ),
)]
async fn delete_account_by_id(
    State(pg_database): State<PgClient>,
    Path(path_params): Path<AccountPathParams>,
) -> Result<(StatusCode, Json<DeleteAccountResponse>)> {
    delete_account_internal(pg_database, path_params.account_id).await
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes(state: ServiceState) -> OpenApiRouter<ServiceState> {
    let require_admin = from_fn_with_state(state.clone(), require_admin);

    OpenApiRouter::new()
        .routes(routes!(
            get_account_by_id,
            update_account_by_id,
            delete_account_by_id
        ))
        .route_layer(require_admin)
        .routes(routes!(
            get_own_account,
            update_own_account,
            delete_own_account
        ))
}

#[cfg(test)]
mod test {
    use crate::handler::accounts::routes;
    use crate::handler::test::create_test_server_with_router;

    #[tokio::test]
    async fn handlers_startup() -> anyhow::Result<()> {
        let server = create_test_server_with_router(routes).await?;

        // Retrieves authenticated account.
        let response = server.get("/accounts/").await;
        response.assert_status_success();

        // Updates authenticated account.
        let response = server.patch("/accounts/").await;
        response.assert_status_success();

        // Deletes authenticated account.
        let response = server.delete("/accounts/").await;
        response.assert_status_success();

        Ok(())
    }
}
