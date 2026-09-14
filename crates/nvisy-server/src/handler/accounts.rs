//! Account management handlers for user profile operations.
//!
//! These handlers are thin: they authenticate, parse the request, delegate the
//! account rules to [`AccountService`](crate::domain::AccountService), and map the
//! result to a response. Avatar upload/delete stay here over
//! [`AvatarService`](crate::service::AvatarService), with the self-authorization
//! check inline.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use nvisy_postgres::model::Account as AccountModel;
use uuid::Uuid;

use super::request::{AccountPathParams, UpdateAccount};
use super::response::{Account, PublicAccount};
use crate::domain::AccountService;
use crate::extract::{AuthState, AvatarUpload, Json, Path, ValidateJson};
use crate::response::{ErrorKind, ErrorResponse, Result};
use crate::service::{AvatarService, MAX_AVATAR_UPLOAD_BYTES, ServiceState};

/// Tracing target for account operations.
const TRACING_TARGET: &str = "nvisy_server::handler::accounts";

/// Retrieves the authenticated account.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn get_own_account(
    State(accounts): State<AccountService>,
    auth_state: AuthState,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading account");

    let account = accounts.find(auth_state.account_id).await?;

    tracing::info!(target: TRACING_TARGET, "Account read");
    Ok((StatusCode::OK, Json(Account::from_model(account))))
}

fn get_own_account_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get own account")
        .description("Returns the authenticated user's account details.")
        .response::<200, Json<Account>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Retrieves the public profile of an account by its id.
///
/// The requester must share at least one workspace with the target account;
/// otherwise the account is reported as not found. Only public fields are
/// returned — private details (email) are available solely through the
/// caller's own `/account/` view.
#[tracing::instrument(skip_all, fields(requester_id = %auth_state.account_id))]
async fn get_account(
    State(accounts): State<AccountService>,
    auth_state: AuthState,
    Path(path_params): Path<AccountPathParams>,
) -> Result<(StatusCode, Json<PublicAccount>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading account by id");

    let account = accounts
        .find_public(auth_state.account_id, path_params.account_id)
        .await?;

    tracing::info!(target: TRACING_TARGET, "Account read by id");
    Ok((StatusCode::OK, Json(PublicAccount::from_model(account))))
}

fn get_account_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get account by id")
        .description(
            "Returns an account's public profile by its id. \
             The requester must share at least one workspace with the target account.",
        )
        .response::<200, Json<PublicAccount>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates the authenticated account.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn update_own_account(
    State(accounts): State<AccountService>,
    auth_state: AuthState,
    ValidateJson(request): ValidateJson<UpdateAccount>,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating account");

    let account = accounts
        .update(auth_state.account_id, request.into_model())
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
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn delete_own_account(
    State(accounts): State<AccountService>,
    auth_state: AuthState,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting account");

    accounts.delete(auth_state.account_id).await?;

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

/// Uploads (or replaces) the authenticated account's avatar.
///
/// The image is normalized to WebP and stored; the account's `avatar_url` is set
/// to its serve path. Only the account itself may set its avatar, so the
/// `{accountId}` in the path must be the caller's own. Requires a multipart body
/// with an image field.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn upload_account_avatar(
    State(accounts): State<AccountService>,
    State(avatar): State<AvatarService>,
    auth_state: AuthState,
    Path(path_params): Path<AccountPathParams>,
    AvatarUpload(bytes): AvatarUpload,
) -> Result<(StatusCode, Json<Account>)> {
    tracing::debug!(target: TRACING_TARGET, "Uploading account avatar");

    let account = accounts.find(auth_state.account_id).await?;
    authorize_self(&account, path_params.account_id)?;

    let updated = avatar.set_account_avatar(account.id, bytes).await?;

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
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn delete_account_avatar(
    State(accounts): State<AccountService>,
    State(avatar): State<AvatarService>,
    auth_state: AuthState,
    Path(path_params): Path<AccountPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Deleting account avatar");

    let account = accounts.find(auth_state.account_id).await?;
    authorize_self(&account, path_params.account_id)?;

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

/// Rejects the request unless the path's account id is the caller's own.
fn authorize_self(account: &AccountModel, account_id: Uuid) -> Result<()> {
    if account.id == account_id {
        Ok(())
    } else {
        Err(ErrorKind::Forbidden.with_message("You can only manage your own avatar"))
    }
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes(_state: ServiceState) -> ApiRouter<ServiceState> {
    use aide::axum::routing::{get_with, put_with};

    ApiRouter::new()
        .api_route(
            "/account/",
            get_with(get_own_account, get_own_account_docs)
                .patch_with(update_own_account, update_own_account_docs)
                .delete_with(delete_own_account, delete_own_account_docs),
        )
        .api_route(
            "/accounts/{accountId}/",
            get_with(get_account, get_account_docs),
        )
        .api_route(
            "/accounts/{accountId}/avatar/",
            put_with(upload_account_avatar, upload_account_avatar_docs)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_UPLOAD_BYTES))
                .delete_with(delete_account_avatar, delete_account_avatar_docs),
        )
        .with_path_items(|item| item.tag("Accounts"))
}
