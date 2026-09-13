//! API token management handlers for user API token operations.
//!
//! These handlers are thin: they authenticate, parse the request, delegate the
//! token rules to [`AccountApiTokenService`](crate::domain::AccountApiTokenService),
//! and map the result to a response.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;

use super::request::{
    AccountApiTokenPathParams, CreateAccountApiToken, CursorPagination, UpdateAccountApiToken,
};
use super::response::{AccountApiToken, AccountApiTokenWithJwt, AccountApiTokensPage};
use crate::domain::{AccountApiTokenService, output};
use crate::extract::{AuthState, Json, Path, Query, SecurityContext, ValidateJson};
use crate::response::{ErrorResponse, Result};
use crate::service::ServiceState;

/// Tracing target for API token operations.
const TRACING_TARGET: &str = "nvisy_server::handler::tokens";

/// Creates a new API token for the authenticated account.
///
/// Returns the token with a JWT that can be used for authentication.
/// The JWT is only shown once upon creation.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn create_api_token(
    State(tokens): State<AccountApiTokenService>,
    auth_state: AuthState,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<CreateAccountApiToken>,
) -> Result<(StatusCode, Json<AccountApiTokenWithJwt>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating API token");

    let new_token = request.into_model(auth_state.account_id, security)?;
    let output::CreatedApiToken { token, jwt } =
        tokens.create(auth_state.account_id, new_token).await?;

    let response = AccountApiToken::from_model(token).with_jwt(jwt);
    Ok((StatusCode::CREATED, Json(response)))
}

fn create_api_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create API token")
        .description("Creates a new API token. The JWT token is only shown once upon creation.")
        .response::<201, Json<AccountApiTokenWithJwt>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Lists API tokens for the authenticated account.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn list_api_tokens(
    State(tokens): State<AccountApiTokenService>,
    auth_state: AuthState,
    Query(pagination): Query<CursorPagination>,
) -> Result<(StatusCode, Json<AccountApiTokensPage>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing API tokens");

    let page = tokens
        .list(auth_state.account_id, pagination.into_cursor())
        .await?;

    // Flag the token this request authenticated with so the client can single
    // out the current session.
    let current_token_id = auth_state.token_id;
    let response = AccountApiTokensPage::from_cursor_page(page, |token| {
        let is_current = token.id == current_token_id;
        AccountApiToken::from_model(token).with_current(is_current)
    });

    Ok((StatusCode::OK, Json(response)))
}

fn list_api_tokens_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List API tokens")
        .description("Returns all API tokens for the authenticated account.")
        .response::<200, Json<AccountApiTokensPage>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Gets a specific API token by ID.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn read_api_token(
    State(tokens): State<AccountApiTokenService>,
    auth_state: AuthState,
    Path(path): Path<AccountApiTokenPathParams>,
) -> Result<(StatusCode, Json<AccountApiToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading API token");

    let token = tokens.read(auth_state.account_id, path.token_id).await?;

    let is_current = token.id == auth_state.token_id;
    Ok((
        StatusCode::OK,
        Json(AccountApiToken::from_model(token).with_current(is_current)),
    ))
}

fn read_api_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get API token")
        .description("Returns details for a specific API token.")
        .response::<200, Json<AccountApiToken>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates an existing API token.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn update_api_token(
    State(tokens): State<AccountApiTokenService>,
    auth_state: AuthState,
    Path(path): Path<AccountApiTokenPathParams>,
    ValidateJson(request): ValidateJson<UpdateAccountApiToken>,
) -> Result<(StatusCode, Json<AccountApiToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating API token");

    let updated = tokens
        .update(auth_state.account_id, path.token_id, request.display_name)
        .await?;

    let is_current = updated.id == auth_state.token_id;
    Ok((
        StatusCode::OK,
        Json(AccountApiToken::from_model(updated).with_current(is_current)),
    ))
}

fn update_api_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update API token")
        .description("Updates an existing API token's name.")
        .response::<200, Json<AccountApiToken>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<403, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Revokes (soft deletes) an API token.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn revoke_api_token(
    State(tokens): State<AccountApiTokenService>,
    auth_state: AuthState,
    Path(path): Path<AccountApiTokenPathParams>,
) -> Result<StatusCode> {
    tracing::debug!(target: TRACING_TARGET, "Revoking API token");

    tokens.revoke(auth_state.account_id, path.token_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn revoke_api_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Revoke API token")
        .description("Revokes an API token. This action cannot be undone.")
        .response::<204, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Returns routes for API token management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route(
            "/api-tokens/",
            post_with(create_api_token, create_api_token_docs)
                .get_with(list_api_tokens, list_api_tokens_docs),
        )
        .api_route(
            "/api-tokens/{tokenId}/",
            get_with(read_api_token, read_api_token_docs)
                .patch_with(update_api_token, update_api_token_docs)
                .delete_with(revoke_api_token, revoke_api_token_docs),
        )
        .with_path_items(|item| item.tag("API Tokens"))
}
