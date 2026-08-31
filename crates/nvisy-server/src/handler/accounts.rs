//! Account management handlers for user profile and notification operations.
//!
//! This module provides comprehensive account management functionality including
//! profile viewing, updating, deletion, and notifications. All operations follow
//! security best practices with proper authorization and input validation.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use nvisy_postgres::model::Account as AccountModel;
use nvisy_postgres::query::{AccountRepository, WorkspaceMemberRepository};
use nvisy_postgres::{PgClient, PgConn};
use uuid::Uuid;

use super::request::{AccountPathParams, UpdateAccount};
use super::response::{Account, ErrorResponse, PublicAccount};
use crate::extract::{AuthState, Avatar, Json, Path, ValidateJson};
use crate::handler::utility::build_password_user_inputs;
use crate::handler::{Error, ErrorKind, Result};
use crate::service::{AvatarService, MAX_AVATAR_UPLOAD_BYTES, PasswordService, ServiceState};

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
        target_id = tracing::field::Empty,
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
    tracing::Span::current().record("target_id", tracing::field::display(account.id));

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
        Some(change) => {
            // Re-authenticate the change: it must prove knowledge of the current
            // password, so a hijacked session or CSRF cannot silently reset it
            // (and lock out the real owner).
            if password
                .verify(&change.current_password, &current_account.password_hash)
                .is_err()
            {
                tracing::warn!(target: TRACING_TARGET, "Password change failed: current password incorrect");
                return Err(ErrorKind::Unauthorized
                    .with_message("Current password is incorrect")
                    .with_resource("account"));
            }

            let new_password = &change.new_password;
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
            .with_message("Handle is already taken")
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

/// Finds an account by ID or returns NotFound error.
/// Uploads (or replaces) the authenticated account's avatar.
///
/// The image is normalized to WebP and stored; the account's `avatar_url` is set
/// to its serve path. Only the account itself may set its avatar, so the
/// `{username}` in the path must resolve to the caller. Requires a multipart body
/// with an image field.
#[tracing::instrument(skip_all, fields(account_id = %auth_claims.account_id))]
async fn upload_account_avatar(
    State(pg_client): State<PgClient>,
    State(avatar): State<AvatarService>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<AccountPathParams>,
    Avatar(bytes): Avatar,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::debug!(target: TRACING_TARGET, "Uploading account avatar");

    // Authorize under a scoped connection, then release it: `set_account_avatar`
    // does image processing and a NATS put (and acquires its own connection for
    // the DB update), so holding this one across it would pin two pooled
    // connections for the whole upload.
    let account_id = {
        let mut conn = pg_client.get_connection().await?;
        let account = find_account(&mut conn, auth_claims.account_id).await?;
        authorize_self(&account, &path_params.username)?;
        account.id
    };

    let updated = avatar.set_account_avatar(account_id, bytes).await?;

    tracing::info!(target: TRACING_TARGET, "Account avatar set");
    Ok((StatusCode::OK, Json(Account::from_model(updated))))
}

fn upload_account_avatar_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Upload account avatar")
        .description(
            "Uploads and normalizes the account's avatar. Only the account itself may set it.",
        )
        .response::<200, Json<Account>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Removes the authenticated account's avatar. Only the account itself may.
#[tracing::instrument(skip_all, fields(account_id = %auth_claims.account_id))]
async fn delete_account_avatar(
    State(pg_client): State<PgClient>,
    State(avatar): State<AvatarService>,
    AuthState(auth_claims): AuthState,
    Path(path_params): Path<AccountPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting account avatar");

    let mut conn = pg_client.get_connection().await?;
    let account = find_account(&mut conn, auth_claims.account_id).await?;
    authorize_self(&account, &path_params.username)?;

    avatar.delete_account_avatar(account.id).await?;
    tracing::info!(target: TRACING_TARGET, "Account avatar deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn delete_account_avatar_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete account avatar")
        .description("Removes the account's avatar. Only the account itself may delete it.")
        .response::<204, ()>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Rejects the request unless the path's username resolves to `account`.
fn authorize_self(account: &AccountModel, username: &nvisy_postgres::types::Handle) -> Result<()> {
    if &account.username == username {
        Ok(())
    } else {
        Err(ErrorKind::Forbidden.with_message("You can only manage your own avatar"))
    }
}

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
        .api_route(
            "/accounts/{username}/avatar/",
            put_with(upload_account_avatar, upload_account_avatar_docs)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_UPLOAD_BYTES))
                .delete_with(delete_account_avatar, delete_account_avatar_docs),
        )
        .with_path_items(|item| item.tag("Accounts"))
}
