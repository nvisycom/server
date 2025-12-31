//! API token management handlers for user API token operations.
//!
//! This module provides comprehensive API token management functionality including
//! creation, listing, updating, and revoking. All operations follow security best
//! practices with proper authorization, input validation, and audit logging.

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use axum_extra::headers::UserAgent;
use nvisy_postgres::model::UpdateAccountApiToken;
use nvisy_postgres::query::{
    AccountApiTokenRepository, AccountRepository, Pagination as QueryPagination,
};
use uuid::Uuid;

use super::request::{CreateApiToken, Pagination, TokenPathParams, UpdateApiToken};
use super::response::{ApiToken, ApiTokenWithJWT, ApiTokens, ErrorResponse};
use crate::extract::{
    AuthClaims, AuthHeader, AuthState, Json, Path, PgPool, Query, TypedHeader, ValidateJson,
};
use crate::handler::{ErrorKind, Result};
use crate::service::{AuthKeys, ServiceState};

/// Tracing target for API token operations.
const TRACING_TARGET: &str = "nvisy_server::handler::tokens";

/// Creates a new API token for the authenticated account.
///
/// Returns the token with a JWT that can be used for authentication.
/// The JWT is only shown once upon creation.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn create_api_token(
    PgPool(mut conn): PgPool,
    State(auth_keys): State<AuthKeys>,
    AuthState(auth_state): AuthState,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<CreateApiToken>,
) -> Result<(StatusCode, Json<ApiTokenWithJWT>)> {
    tracing::debug!(target: TRACING_TARGET, "Creating API token");

    // Fetch the account to generate JWT claims
    let account = conn
        .find_account_by_id(auth_state.account_id)
        .await?
        .ok_or_else(|| {
            ErrorKind::NotFound
                .with_resource("account")
                .with_message("Account not found")
        })?;

    let new_token = request.into_model(auth_state.account_id, user_agent.to_string())?;
    let api_token = conn.create_token(new_token).await?;

    // Generate JWT for the new token
    let auth_claims = AuthClaims::new(&account, &api_token);
    let auth_header = AuthHeader::new(auth_claims, auth_keys);
    let jwt_token = auth_header.into_string()?;

    let response = ApiToken::from_model(api_token.clone()).with_jwt(jwt_token);

    tracing::info!(
        target: TRACING_TARGET,
        token_id = %api_token.id,
        "API token created",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

fn create_api_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create API token")
        .description("Creates a new API token. The JWT token is only shown once upon creation.")
        .response::<201, Json<ApiTokenWithJWT>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Lists API tokens for the authenticated account.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn list_api_tokens(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Query(pagination): Query<Pagination>,
) -> Result<(StatusCode, Json<ApiTokens>)> {
    tracing::debug!(target: TRACING_TARGET, "Listing API tokens");

    // Validate pagination parameters
    if let Some(limit) = pagination.limit {
        if limit == 0 {
            return Err(ErrorKind::BadRequest
                .with_resource("pagination")
                .with_message("Limit must be greater than 0"));
        }

        if limit > 100 {
            return Err(ErrorKind::BadRequest
                .with_resource("pagination")
                .with_message("Limit cannot exceed 100"));
        }
    }

    let tokens = conn
        .list_account_tokens(auth_state.account_id, QueryPagination::from(pagination))
        .await?;

    let api_tokens: ApiTokens = ApiToken::from_models(tokens);

    tracing::debug!(
        target: TRACING_TARGET,
        count = api_tokens.len(),
        "API tokens listed",
    );

    Ok((StatusCode::OK, Json(api_tokens)))
}

fn list_api_tokens_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List API tokens")
        .description("Returns all API tokens for the authenticated account.")
        .response::<200, Json<ApiTokens>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Gets a specific API token by ID.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn read_api_token(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path): Path<TokenPathParams>,
) -> Result<(StatusCode, Json<ApiToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading API token");

    let token = find_account_token(&mut conn, auth_state.account_id, path.token_id).await?;

    tracing::debug!(target: TRACING_TARGET, "API token read");

    Ok((StatusCode::OK, Json(ApiToken::from_model(token))))
}

fn read_api_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get API token")
        .description("Returns details for a specific API token.")
        .response::<200, Json<ApiToken>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Updates an existing API token.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn update_api_token(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path): Path<TokenPathParams>,
    ValidateJson(request): ValidateJson<UpdateApiToken>,
) -> Result<(StatusCode, Json<ApiToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Updating API token");

    // Verify the token exists and belongs to the authenticated account
    let token = find_account_token(&mut conn, auth_state.account_id, path.token_id).await?;

    let update_token = UpdateAccountApiToken {
        name: request.name,
        ..Default::default()
    };

    let updated_token = conn.update_token_by_id(token.id, update_token).await?;

    tracing::info!(target: TRACING_TARGET, "API token updated");

    Ok((StatusCode::OK, Json(ApiToken::from_model(updated_token))))
}

fn update_api_token_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update API token")
        .description("Updates an existing API token's name.")
        .response::<200, Json<ApiToken>>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
        .response::<404, Json<ErrorResponse>>()
}

/// Revokes (soft deletes) an API token.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn revoke_api_token(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(path): Path<TokenPathParams>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Revoking API token");

    // Verify the token exists and belongs to the authenticated account
    let token = find_account_token(&mut conn, auth_state.account_id, path.token_id).await?;

    let deleted = conn.delete_token_by_id(token.id).await?;

    if !deleted {
        return Err(ErrorKind::BadRequest
            .with_resource("api_token")
            .with_message("API token is already revoked"));
    }

    tracing::warn!(target: TRACING_TARGET, "API token revoked");

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

/// Finds an API token by ID and verifies it belongs to the specified account.
async fn find_account_token(
    conn: &mut nvisy_postgres::PgConn,
    account_id: Uuid,
    token_id: Uuid,
) -> Result<nvisy_postgres::model::AccountApiToken> {
    let Some(token) = conn.find_token_by_id(token_id).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found"));
    };

    if token.account_id != account_id {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found"));
    }

    Ok(token)
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
            "/api-tokens/{token_id}/",
            get_with(read_api_token, read_api_token_docs)
                .patch_with(update_api_token, update_api_token_docs)
                .delete_with(revoke_api_token, revoke_api_token_docs),
        )
        .with_path_items(|item| item.tag("API Tokens"))
}
