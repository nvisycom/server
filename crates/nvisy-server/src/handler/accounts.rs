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
use nvisy_postgres::PgClient;
use nvisy_postgres::model::Account as AccountModel;
use nvisy_postgres::query::{AccountNotificationRepository, AccountRepository};
use uuid::Uuid;

use super::request::{CursorPagination, UpdateAccount};
use super::response::{Account, ErrorResponse, Notification, NotificationsPage, UnreadStatus};
use crate::extract::{AuthState, Json, Query, ValidateJson};
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

    let mut conn = pg_client.get_connection().await?;
    let account = find_account(&mut conn, auth_claims.account_id).await?;

    tracing::info!(target: TRACING_TARGET, "Account read");

    Ok((StatusCode::OK, Json(Account::from_model(account))))
}

fn get_own_account_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get account")
        .description("Returns the authenticated user's account details.")
        .response::<200, Json<Account>>()
        .response::<401, Json<ErrorResponse>>()
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
    tracing::debug!(target: TRACING_TARGET, "Updating account");

    let mut conn = pg_client.get_connection().await?;
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
    if let Some(ref email) = request.email_address
        && conn
            .email_exists_for_other(email, auth_claims.account_id)
            .await?
    {
        tracing::warn!(target: TRACING_TARGET, "Account update failed: email already exists");
        return Err(ErrorKind::Conflict
            .with_message("Account with this email already exists")
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
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_message("Account not found.")
                .with_resource("account")
        })?;

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

/// Lists notifications for the authenticated account and marks them as read.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_state.account_id)
)]
async fn list_notifications(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<NotificationsPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing notifications");

    let mut conn = pg_client.get_connection().await?;

    let page = conn
        .cursor_list_account_notifications(auth_state.account_id, pagination.into())
        .await?;

    // Mark all unread notifications as read
    let unread_count = conn
        .mark_all_account_notifications_as_read(auth_state.account_id)
        .await?;

    if unread_count > 0 {
        tracing::debug!(
            target: TRACING_TARGET,
            unread_count,
            "Marked notifications as read"
        );
    }

    let response = NotificationsPage::from_cursor_page(page, Notification::from_model);

    tracing::debug!(
        target: TRACING_TARGET,
        notification_count = response.items.len(),
        "Notifications listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

fn list_notifications_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List notifications")
        .description(
            "Returns all notifications for the authenticated account and marks them as read.",
        )
        .response::<200, Json<NotificationsPage>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Returns the count of unread notifications for the authenticated account.
#[tracing::instrument(
    skip_all,
    fields(account_id = %auth_state.account_id)
)]
async fn get_unread_status(
    State(pg_client): State<PgClient>,
    AuthState(auth_state): AuthState,
) -> Result<(StatusCode, Json<UnreadStatus>)> {
    tracing::debug!(target: TRACING_TARGET, "Checking unread notifications count");

    let mut conn = pg_client.get_connection().await?;

    let unread_count = conn
        .count_unread_account_notifications(auth_state.account_id)
        .await?;

    tracing::debug!(
        target: TRACING_TARGET,
        unread_count,
        "Unread notifications count retrieved"
    );

    Ok((StatusCode::OK, Json(UnreadStatus { unread_count })))
}

fn get_unread_status_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get unread notifications count")
        .description("Returns the number of unread notifications for the authenticated account.")
        .response::<200, Json<UnreadStatus>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Builds user inputs for password strength validation.
fn build_password_user_inputs<'a>(display_name: &'a str, email_address: &'a str) -> Vec<&'a str> {
    let mut inputs = vec![display_name];
    inputs.extend(email_address.split('@'));
    inputs
}

/// Finds an account by ID or returns NotFound error.
async fn find_account(conn: &mut nvisy_postgres::PgConn, account_id: Uuid) -> Result<AccountModel> {
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
        .api_route(
            "/account",
            get_with(get_own_account, get_own_account_docs)
                .patch_with(update_own_account, update_own_account_docs)
                .delete_with(delete_own_account, delete_own_account_docs),
        )
        .api_route(
            "/notifications/",
            get_with(list_notifications, list_notifications_docs),
        )
        .api_route(
            "/notifications/unread",
            get_with(get_unread_status, get_unread_status_docs),
        )
        .with_path_items(|item| item.tag("Accounts"))
}
