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
#[tracing::instrument(skip_all)]
async fn get_own_account(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        "retrieving own account"
    );

    let Some(account) = pg_client.find_account_by_id(auth_claims.account_id).await? else {
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

    let account = Account::from_model(account);
    Ok((StatusCode::OK, Json(account)))
}

/// Updates the authenticated account.
#[tracing::instrument(skip_all)]
async fn update_own_account(
    State(pg_client): State<PgClient>,
    State(auth_hasher): State<PasswordHasher>,
    State(password_strength): State<PasswordStrength>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<UpdateAccount>,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        has_display_name = request.display_name.is_some(),
        has_email = request.email_address.is_some(),
        has_password = request.password.is_some(),
        "updating account"
    );

    // Get current account info for password validation
    let Some(current_account) = pg_client.find_account_by_id(auth_claims.account_id).await? else {
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
        && pg_client.email_exists(email).await?
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

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account.id,
        "account updated"
    );

    let account = Account::from_model(account);
    Ok((StatusCode::OK, Json(account)))
}

/// Deletes the authenticated account.
#[tracing::instrument(skip_all)]
async fn delete_own_account(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        "deleting own account"
    );

    pg_client.delete_account(auth_claims.account_id).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        "account deleted"
    );

    Ok(StatusCode::OK)
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

