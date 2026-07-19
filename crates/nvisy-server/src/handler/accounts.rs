//! Account management handlers for user profile and notification operations.
//!
//! This module provides comprehensive account management functionality including
//! profile viewing, updating, deletion, and notifications. All operations follow
//! security best practices with proper authorization, input validation, and audit
//! logging.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use nvisy_postgres::model::Account as AccountModel;
use nvisy_postgres::query::{AccountRepository, WorkspaceMemberRepository};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use super::request::{AccountPathParams, UpdateAccount};
use super::response::{Account, ErrorResponse, PublicAccount};
use crate::extract::{AuthState, Json, Path, ValidateJson};
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{PasswordService, ServiceState};

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

    let mut conn = pg_client.get_connection().await?;
    let account = find_account(&mut conn, auth_claims.account_id).await?;

    tracing::info!(target: TRACING_TARGET, "Account read");

    Ok((StatusCode::OK, Json(Account::from_model(account))))
}

fn get_own_account_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get own account")
        .description("Returns the authenticated user's account details.")
        .response::<200, Json<Account>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Retrieves the public profile of an account by its handle.
///
/// The requester must share at least one workspace with the target account;
/// otherwise the account is reported as not found. Only public fields are
/// returned — private details (email) are available solely through the
/// caller's own `/account/` view.
#[tracing::instrument(
    skip_all,
    fields(
        requester_id = %auth_claims.account_id,
        target = %path_params.username,
    )
)]
async fn get_account(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<AccountPathParams>,
) -> Result<(StatusCode, Json<PublicAccount>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading account by username");

    let mut conn = pg_client.get_connection().await?;

    let account = conn
        .find_account_by_username(&path_params.username)
        .await?
        .ok_or_else(|| Error::not_found("account"))?;

    // Accessible only to accounts that share a workspace. A non-shared account is
    // reported as not-found (not forbidden) so this endpoint cannot be used to
    // distinguish existing from non-existing handles.
    let shares_workspace = conn
        .accounts_share_workspace(auth_claims.account_id, account.id)
        .await?;

    if !shares_workspace {
        tracing::warn!(
            target: TRACING_TARGET,
            "Account not accessible: no shared workspace"
        );
        return Err(Error::not_found("account"));
    }

    tracing::info!(target: TRACING_TARGET, "Account read by username");

    Ok((StatusCode::OK, Json(PublicAccount::from_model(account))))
}

fn get_account_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get account by username")
        .description(
            "Returns an account's public profile by its handle. \
             The requester must share at least one workspace with the target account.",
        )
        .response::<200, Json<PublicAccount>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_claims.account_id)
)]
async fn update_own_account(
    State(pg_client): State<PgClient>,
    State(password): State<PasswordService>,
    AuthState(auth_claims): AuthState,
    ValidateJson(request): ValidateJson<UpdateAccount>,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating account");

    let mut conn = pg_client.get_connection().await?;
    let current_account = find_account(&mut conn, auth_claims.account_id).await?;

    // Validate and hash password if provided
    let password_hash = match request.password.as_ref() {
        Some(new_password) => {
            let username = request
                .username
                .as_ref()
                .unwrap_or(&current_account.username)
                .as_str();
            let display_name = request
                .display_name
                .as_deref()
                .or(current_account.display_name.as_deref());
            let email_address = request
                .email_address
                .as_deref()
                .unwrap_or(&current_account.email_address);

            let user_inputs = build_password_user_inputs(username, display_name, email_address);
            Some(password.validate_and_hash(new_password, &user_inputs)?)
        }
        None => None,
    };

    // Check if email already exists for another account
    if let Some(ref email) = request.email_address
        && conn
            .email_exists_for_other(email, auth_claims.account_id)
            .await?
    {
        tracing::warn!(target: TRACING_TARGET, "Account update failed: email already exists");
        return Err(ErrorKind::Conflict
            .with_message("Email is already registered")
            .with_resource("account"));
    }

    // Check if username is already taken by another account
    if let Some(ref username) = request.username
        && conn
            .username_exists_for_other(username, auth_claims.account_id)
            .await?
    {
        tracing::warn!(target: TRACING_TARGET, "Account update failed: username already taken");
        return Err(ErrorKind::Conflict
            .with_message("Username is already taken")
            .with_resource("account"));
    }

    let account = conn
        .update_account(auth_claims.account_id, request.into_model(password_hash))
        .await?;

    tracing::info!(target: TRACING_TARGET, "Account updated");

    Ok((StatusCode::OK, Json(Account::from_model(account))))
}

fn update_own_account_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update account")
        .description("Updates the authenticated user's account details.")
        .response::<200, Json<Account>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
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
    tracing::debug!(target: TRACING_TARGET, "Deleting account");

    let mut conn = pg_client.get_connection().await?;
    conn.delete_account(auth_claims.account_id)
        .await?
        .ok_or_else(|| Error::not_found("account"))?;

    tracing::info!(target: TRACING_TARGET, "Account deleted");

    Ok(StatusCode::OK)
}

fn delete_own_account_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete account")
        .description("Deletes the authenticated user's account.")
        .response_with::<200, (), _>(|res| res.description("Account deleted."))
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Builds user inputs for password strength validation.
fn build_password_user_inputs<'a>(
    username: &'a str,
    display_name: Option<&'a str>,
    email_address: &'a str,
) -> Vec<&'a str> {
    let mut inputs = vec![username];
    inputs.extend(display_name);
    inputs.extend(email_address.split('@'));
    inputs
}

/// Finds an account by ID or returns NotFound error.
async fn find_account(conn: &mut PgConn, account_id: Uuid) -> Result<AccountModel> {
    conn.find_account_by_id(account_id)
        .await?
        .ok_or_else(|| Error::not_found("account"))
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes(_state: ServiceState) -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/account/",
            get_with(get_own_account, get_own_account_docs)
                .patch_with(update_own_account, update_own_account_docs)
                .delete_with(delete_own_account, delete_own_account_docs),
        )
        .api_route(
            "/accounts/{username}/",
            get_with(get_account, get_account_docs),
        )
        .with_path_items(|item| item.tag("Accounts"))
}
