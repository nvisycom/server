//! API token management handlers for user API token operations.
//!
//! This module provides comprehensive API token management functionality including
//! creation, listing, updating, and revoking. All operations follow security best
//! practices with proper authorization, input validation, and audit logging.

use aide::axum::ApiRouter;
use axum::http::StatusCode;
use axum_extra::headers::UserAgent;
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use jiff::Timestamp;
use nvisy_postgres::model::UpdateAccountApiToken;
use nvisy_postgres::query::{AccountApiTokenRepository, Pagination as QueryPagination};
use uuid::Uuid;

use super::request::{CreateApiToken, Pagination, UpdateApiToken};
use super::response::{ApiToken, ApiTokenWithSecret, ApiTokens};
use crate::extract::{AuthState, ClientIp, Json, Path, PgPool, Query, TypedHeader, ValidateJson};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for API token operations.
const TRACING_TARGET: &str = "nvisy_server::handler::tokens";

/// Creates a new API token for the authenticated account.
///
/// Returns the token with full access and refresh tokens. These are only shown once.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn create_api_token(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    ClientIp(ip_address): ClientIp,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<CreateApiToken>,
) -> Result<(StatusCode, Json<ApiTokenWithSecret>)> {
    tracing::info!(target: TRACING_TARGET, "Creating API token");

    let new_token =
        request.into_model(auth_state.account_id, ip_address, user_agent.to_string())?;
    let token = conn.create_token(new_token).await?;

    tracing::info!(
        target: TRACING_TARGET,
        token_id = %token.access_seq_short(),
        "API token created successfully",
    );

    Ok((StatusCode::CREATED, Json(token.into())))
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

    let api_tokens: ApiTokens = tokens.into_iter().map(ApiToken::from).collect();

    tracing::debug!(
        target: TRACING_TARGET,
        count = api_tokens.len(),
        "API tokens listed successfully",
    );

    Ok((StatusCode::OK, Json(api_tokens)))
}

/// Gets a specific API token by access token.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn read_api_token(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(access_token): Path<Uuid>,
) -> Result<(StatusCode, Json<ApiToken>)> {
    tracing::debug!(target: TRACING_TARGET, "Reading API token");

    let token = find_account_token(&mut conn, auth_state.account_id, access_token).await?;

    tracing::debug!(target: TRACING_TARGET, "API token retrieved successfully");

    Ok((StatusCode::OK, Json(token.into())))
}

/// Updates an existing API token.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn update_api_token(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(access_token): Path<Uuid>,
    ValidateJson(request): ValidateJson<UpdateApiToken>,
) -> Result<(StatusCode, Json<ApiToken>)> {
    tracing::info!(target: TRACING_TARGET, "Updating API token");

    // Verify the token exists and belongs to the authenticated account
    let _ = find_account_token(&mut conn, auth_state.account_id, access_token).await?;

    let update_token = UpdateAccountApiToken {
        last_used_at: Some(Timestamp::now().into()),
        name: request.name,
        description: request.description,
        ..Default::default()
    };

    let updated_token = conn.update_token(access_token, update_token).await?;

    tracing::info!(target: TRACING_TARGET, "API token updated successfully");

    Ok((StatusCode::OK, Json(updated_token.into())))
}

/// Revokes (soft deletes) an API token.
#[tracing::instrument(skip_all, fields(account_id = %auth_state.account_id))]
async fn revoke_api_token(
    PgPool(mut conn): PgPool,
    AuthState(auth_state): AuthState,
    Path(access_token): Path<Uuid>,
) -> Result<StatusCode> {
    tracing::warn!(target: TRACING_TARGET, "Revoking API token");

    // Verify the token exists and belongs to the authenticated account
    let _ = find_account_token(&mut conn, auth_state.account_id, access_token).await?;

    let deleted = conn.delete_token(access_token).await?;

    if !deleted {
        return Err(ErrorKind::BadRequest
            .with_resource("api_token")
            .with_message("API token is already revoked"));
    }

    tracing::warn!(target: TRACING_TARGET, "API token revoked successfully");

    Ok(StatusCode::NO_CONTENT)
}

/// Finds an API token and verifies it belongs to the specified account.
async fn find_account_token(
    conn: &mut nvisy_postgres::PgConn,
    account_id: Uuid,
    access_token: Uuid,
) -> Result<nvisy_postgres::model::AccountApiToken> {
    let Some(token) = conn.find_token_by_access_token(access_token).await? else {
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

/// Converts an IP address to an IpNet for storage.
pub(crate) fn ip_to_net(ip: std::net::IpAddr) -> Result<IpNet> {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            Ok(IpNet::V4(Ipv4Net::new(ipv4, 32).map_err(|_| {
                ErrorKind::BadRequest.with_message("Invalid IPv4 address")
            })?))
        }
        std::net::IpAddr::V6(ipv6) => {
            Ok(IpNet::V6(Ipv6Net::new(ipv6, 128).map_err(|_| {
                ErrorKind::BadRequest.with_message("Invalid IPv6 address")
            })?))
        }
    }
}

/// Returns routes for API token management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/api-tokens/", post(create_api_token))
        .api_route("/api-tokens/", get(list_api_tokens))
        .api_route("/api-tokens/{access_token}/", get(read_api_token))
        .api_route("/api-tokens/{access_token}/", patch(update_api_token))
        .api_route("/api-tokens/{access_token}/", delete(revoke_api_token))
}
