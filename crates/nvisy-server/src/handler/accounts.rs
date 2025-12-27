//! Account management handlers for user profile operations.
//!
//! This module provides comprehensive account management functionality including
//! profile viewing, updating, and deletion. All operations follow security best
//! practices with proper authorization, input validation, and audit logging.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::query::AccountRepository;
use nvisy_postgres::{PgClient, model};

use super::request::UpdateAccount;
use super::response::Account;
use crate::extract::{AuthState, Json, ValidateJson};
use crate::handler::{ErrorKind, Result};
use crate::service::{PasswordHasher, PasswordStrength, ServiceState};

/// Tracing target for account operations.
const TRACING_TARGET: &str = "nvisy_server::handler::accounts";

/// Retrieves the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_claims.account_id)
)]
async fn get_own_account(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading account");

    let account = find_account(&pg_client, auth_claims.account_id).await?;

    tracing::debug!(target: TRACING_TARGET, "Account retrieved successfully");

    Ok((StatusCode::OK, Json(Account::from_model(account))))
}

/// Updates the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_claims.account_id)
)]
async fn update_own_account(
    State(pg_client): State<PgClient>,
    State(auth_hasher): State<PasswordHasher>,
    State(password_strength): State<PasswordStrength>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<UpdateAccount>,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::info!(target: TRACING_TARGET, "Updating account");

    let current_account = find_account(&pg_client, auth_claims.account_id).await?;

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
        && pg_client.email_exists(email).await?
        && current_account.email_address != *email
    {
        tracing::warn!(target: TRACING_TARGET, "Account update failed: email already exists");
        return Err(ErrorKind::Conflict
            .with_message("Account with this email already exists")
            .with_resource("account"));
    }

    let update_account = model::UpdateAccount {
        display_name: request.display_name,
        email_address: normalized_email,
        password_hash,
        company_name: request.company_name,
        phone_number: request.phone_number,
        ..Default::default()
    };

    let account = pg_client
        .update_account(auth_claims.account_id, update_account)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Account updated successfully");

    Ok((StatusCode::OK, Json(Account::from_model(account))))
}

/// Deletes the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_claims.account_id)
)]
async fn delete_own_account(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Account deletion requested");

    // Verify account exists
    let _ = find_account(&pg_client, auth_claims.account_id).await?;

    pg_client.delete_account(auth_claims.account_id).await?;

    tracing::warn!(target: TRACING_TARGET, "Account deleted successfully");

    Ok(StatusCode::OK)
}

/// Finds an account by ID or returns NotFound error.
async fn find_account(
    pg_client: &PgClient,
    account_id: uuid::Uuid,
) -> Result<nvisy_postgres::model::Account> {
    pg_client
        .find_account_by_id(account_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Account not found")
                .with_resource("account")
        })
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes(_state: ServiceState) -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/me", get(get_own_account))
        .api_route("/me", patch(update_own_account))
        .api_route("/me", delete(delete_own_account))
}
