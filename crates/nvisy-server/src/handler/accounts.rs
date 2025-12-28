//! Account management handlers for user profile operations.
//!
//! This module provides comprehensive account management functionality including
//! profile viewing, updating, and deletion. All operations follow security best
//! practices with proper authorization, input validation, and audit logging.

use aide::axum::ApiRouter;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{self, Account as AccountModel};
use nvisy_postgres::query::AccountRepository;
use uuid::Uuid;

use super::request::UpdateAccount;
use super::response::Account;
use crate::extract::{AuthState, Json, PgPool, ValidateJson};
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
    PgPool(mut conn): PgPool,
    AuthState(auth_claims): AuthState,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading account");

    let account = find_account(&mut conn, auth_claims.account_id).await?;

    tracing::info!(target: TRACING_TARGET, "Account read");

    Ok((StatusCode::OK, Json(Account::from_model(account))))
}

/// Updates the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_claims.account_id)
)]
async fn update_own_account(
    PgPool(mut conn): PgPool,
    State(auth_hasher): State<PasswordHasher>,
    State(password_strength): State<PasswordStrength>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<UpdateAccount>,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating account");

    let current_account = find_account(&mut conn, auth_claims.account_id).await?;

    // Validate and hash password if provided
    let password_hash = match request.password.as_ref() {
        Some(password) => {
            let display_name = request
                .display_name
                .as_deref()
                .unwrap_or(&current_account.display_name);
            let email_address = request
                .email_address
                .as_deref()
                .unwrap_or(&current_account.email_address);

            let user_inputs = build_password_user_inputs(display_name, email_address);
            password_strength.validate_password(password, &user_inputs)?;

            Some(auth_hasher.hash_password(password)?)
        }
        None => None,
    };

    // Check if email already exists for another account
    if let Some(ref email) = request.email_address {
        let normalized = email.to_lowercase();
        if conn.email_exists(&normalized).await? && current_account.email_address != normalized {
            tracing::warn!(target: TRACING_TARGET, "Account update failed: email already exists");
            return Err(ErrorKind::Conflict
                .with_message("Account with this email already exists")
                .with_resource("account"));
        }
    }

    let update_account = model::UpdateAccount {
        display_name: request.display_name,
        email_address: request.email_address,
        password_hash,
        company_name: request.company_name,
        phone_number: request.phone_number,
        ..Default::default()
    };

    let account = conn
        .update_account(auth_claims.account_id, update_account)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Account updated");

    Ok((StatusCode::OK, Json(Account::from_model(account))))
}

/// Deletes the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_claims.account_id)
)]
async fn delete_own_account(
    PgPool(mut conn): PgPool,
    AuthState(auth_claims): AuthState,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting account");

    conn.delete_account(auth_claims.account_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Account not found.")
                .with_resource("account")
        })?;

    tracing::info!(target: TRACING_TARGET, "Account deleted");

    Ok(StatusCode::OK)
}

/// Builds user inputs for password strength validation.
fn build_password_user_inputs<'a>(display_name: &'a str, email_address: &'a str) -> Vec<&'a str> {
    let mut inputs = vec![display_name];
    inputs.extend(email_address.split('@'));
    inputs
}

/// Finds an account by ID or returns NotFound error.
async fn find_account(conn: &mut PgConn, account_id: Uuid) -> Result<AccountModel> {
    conn.find_account_by_id(account_id).await?.ok_or_else(|| {
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
        .with_path_items(|item| item.tag("Accounts"))
}
