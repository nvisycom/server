//! Account management handlers for user profile operations.
//!
//! This module provides comprehensive account management functionality including
//! profile viewing, updating, and deletion. All operations follow security best
//! practices with proper authorization, input validation, and audit logging.

use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::UpdateAccount;
use nvisy_postgres::query::AccountRepository;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::request::account::UpdateAccountRequest;
use super::response::account::{DeleteAccountResponse, GetAccountResponse, UpdateAccountResponse};
use crate::extract::{AuthState, Json, ValidateJson};
use crate::handler::{ErrorKind, ErrorResponse, Result};
use crate::service::{AuthHasher, PasswordStrength, ServiceState};

/// Tracing target for account operations.
const TRACING_TARGET: &str = "nvisy_server::handler::accounts";

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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<GetAccountResponse>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        "retrieving own account"
    );

    let mut conn = pg_client.get_connection().await?;
    let Some(account) =
        AccountRepository::find_account_by_id(&mut conn, auth_claims.account_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("account")
            .with_message("Account not found")
            .with_context(format!("Account ID: {}", auth_claims.account_id)));
    };

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account.id,
        display_name = %account.display_name,
        "account retrieved"
    );

    Ok((StatusCode::OK, Json(GetAccountResponse::new(account))))
}

/// Updates the authenticated account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    patch, path = "/accounts/", tag = "accounts",
    request_body = UpdateAccountRequest,
    responses(
        (
            status = NOT_FOUND,
            description = "Account not found",
            body = ErrorResponse,
        ),
        (
            status = CONFLICT,
            description = "Email already exists",
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
            description = "Account updated successfully",
        ),
    )
)]
async fn update_own_account(
    State(pg_client): State<PgClient>,
    State(auth_hasher): State<AuthHasher>,
    State(password_strength): State<PasswordStrength>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<UpdateAccountRequest>,
) -> Result<(StatusCode, Json<UpdateAccountResponse>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        has_display_name = request.display_name.is_some(),
        has_email = request.email_address.is_some(),
        has_password = request.password.is_some(),
        "updating account"
    );

    let mut conn = pg_client.get_connection().await?;

    // Get current account info for password validation
    let Some(current_account) =
        AccountRepository::find_account_by_id(&mut conn, auth_claims.account_id).await?
    else {
        return Err(ErrorKind::NotFound
            .with_resource("account")
            .with_message("Account not found")
            .with_context(format!("Account ID: {}", auth_claims.account_id)));
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
            .map_err(|_| {
                ErrorKind::BadRequest
                    .with_message("Password does not meet strength requirements")
                    .with_resource("account")
            })?;

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
            account_id = %auth_claims.account_id,
            email = %email,
            "account update failed: email already exists"
        );
        return Err(ErrorKind::Conflict
            .with_message("Account with this email already exists")
            .with_resource("account"));
    }

    let update_account = UpdateAccount {
        display_name: request.display_name,
        email_address: normalized_email,
        password_hash,
        company_name: request.company_name,
        phone_number: request.phone_number,
        ..Default::default()
    };

    let account =
        AccountRepository::update_account(&mut conn, auth_claims.account_id, update_account)
            .await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account.id,
        "account updated"
    );

    Ok((StatusCode::OK, Json(UpdateAccountResponse::new(account))))
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
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<DeleteAccountResponse>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        "deleting own account"
    );

    let mut conn = pg_client.get_connection().await?;
    let account = AccountRepository::delete_account(&mut conn, auth_claims.account_id).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account.id,
        "account deleted"
    );

    Ok((StatusCode::OK, Json(account.into())))
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes(_state: ServiceState) -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(
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
