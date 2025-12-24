//! API token management handlers for user API token operations.
//!
//! This module provides comprehensive API token management functionality including
//! creation, listing, updating, and revoking. All operations follow security best
//! practices with proper authorization, input validation, and audit logging.

use aide::axum::ApiRouter;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum_extra::headers::UserAgent;
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use jiff::Timestamp;
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewAccountApiToken, UpdateAccountApiToken};
use nvisy_postgres::query::{AccountApiTokenRepository, Pagination as QueryPagination};
use nvisy_postgres::types::ApiTokenType;
use uuid::Uuid;

use super::request::{CreateApiToken, Pagination, UpdateApiToken};
use super::response::{ApiToken, ApiTokenWithSecret, ApiTokens};
use crate::extract::{AuthState, ClientIp, Json, TypedHeader, ValidateJson};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for API token operations.
const TRACING_TARGET: &str = "nvisy_server::handler::api_tokens";

/// Creates a new API token for the authenticated account.
///
/// Returns the token with full access and refresh tokens. These are only shown once.
#[tracing::instrument(skip_all)]
async fn create_api_token(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    ClientIp(ip_address): ClientIp,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<CreateApiToken>,
) -> Result<(StatusCode, Json<ApiTokenWithSecret>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        name = %request.name,
        has_description = request.description.is_some(),
        expires = ?request.expires,
        "creating API token"
    );

    // Sanitize and validate input
    let sanitized_name = request.name.trim().to_string();
    if sanitized_name.is_empty() {
        return Err(ErrorKind::BadRequest
            .with_resource("api_token")
            .with_message("Token name cannot be empty or whitespace only"));
    }

    // Get expiration timestamp from TokenExpiration enum
    let expires_at = request.expires.to_expiry_timestamp();

    // Convert IP address to IpNet for storage
    let ip_net = match ip_address {
        std::net::IpAddr::V4(ipv4) => IpNet::V4(
            Ipv4Net::new(ipv4, 32)
                .map_err(|_| ErrorKind::BadRequest.with_message("Invalid IPv4 address"))?,
        ),
        std::net::IpAddr::V6(ipv6) => IpNet::V6(
            Ipv6Net::new(ipv6, 128)
                .map_err(|_| ErrorKind::BadRequest.with_message("Invalid IPv6 address"))?,
        ),
    };

    let new_token = NewAccountApiToken {
        account_id: auth_claims.account_id,
        name: sanitized_name.clone(),
        description: request.description,
        region_code: None,
        country_code: None,
        city_name: None,
        ip_address: ip_net,
        user_agent: user_agent.to_string(),
        device_id: None,
        session_type: Some(ApiTokenType::Api),
        is_remembered: Some(true),
        expired_at: expires_at.map(Into::into),
    };

    let token = pg_client.create_token(new_token).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        token_preview = %token.access_seq_short(),
        name = %sanitized_name,
        expires_at = ?expires_at,
        "API token created"
    );

    Ok((StatusCode::CREATED, Json(token.into())))
}

/// Lists API tokens for the authenticated account.
#[tracing::instrument(skip_all)]
async fn list_api_tokens(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Query(pagination): Query<Pagination>,
) -> Result<(StatusCode, Json<ApiTokens>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        "listing API tokens"
    );

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

    let pagination = QueryPagination::from(pagination);

    let tokens = pg_client
        .list_account_tokens(auth_claims.account_id, pagination)
        .await?;

    let api_tokens: ApiTokens = tokens.into_iter().map(ApiToken::from).collect();

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        count = api_tokens.len(),
        "API tokens listed"
    );

    Ok((StatusCode::OK, Json(api_tokens)))
}

/// Gets a specific API token by access token.
#[tracing::instrument(skip_all)]
async fn read_api_token(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(access_token): Path<Uuid>,
) -> Result<(StatusCode, Json<ApiToken>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        access_token = %access_token,
        "reading API token"
    );

    let Some(token) = pg_client.find_token_by_access_token(access_token).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found"));
    };

    // Ensure the token belongs to the authenticated account
    if token.account_id != auth_claims.account_id {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found"));
    }

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        token_preview = %token.access_seq_short(),
        "API token retrieved"
    );

    Ok((StatusCode::OK, Json(token.into())))
}

/// Updates an existing API token.
#[tracing::instrument(skip_all)]
async fn update_api_token(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(access_token): Path<Uuid>,
    ValidateJson(request): ValidateJson<UpdateApiToken>,
) -> Result<(StatusCode, Json<ApiToken>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        access_token = %access_token,
        "updating API token"
    );

    // First, verify the token exists and belongs to the authenticated account
    let Some(existing_token) = pg_client.find_token_by_access_token(access_token).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found"));
    };

    if existing_token.account_id != auth_claims.account_id {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found"));
    }

    let update_token = UpdateAccountApiToken {
        last_used_at: Some(Timestamp::now().into()),
        name: request.name,
        description: request.description,
        ..Default::default()
    };

    let updated_token = pg_client.update_token(access_token, update_token).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        token_preview = %existing_token.access_seq_short(),
        "API token updated"
    );

    Ok((StatusCode::OK, Json(updated_token.into())))
}

/// Revokes (soft deletes) an API token.
#[tracing::instrument(skip_all)]
async fn revoke_api_token(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(access_token): Path<Uuid>,
) -> Result<StatusCode> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        access_token = %access_token,
        "revoking API token"
    );

    // First, verify the token exists and belongs to the authenticated account
    let Some(existing_token) = pg_client.find_token_by_access_token(access_token).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found"));
    };

    if existing_token.account_id != auth_claims.account_id {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found"));
    }

    let deleted = pg_client.delete_token(access_token).await?;

    if !deleted {
        return Err(ErrorKind::BadRequest
            .with_resource("api_token")
            .with_message("API token is already revoked"));
    }

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        token_preview = %existing_token.access_seq_short(),
        "API token revoked"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Returns routes for API token management.
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/api-tokens/", post(create_api_token))
        .api_route("/api-tokens/", get(list_api_tokens))
        .api_route("/api-tokens/:access_token/", get(read_api_token))
        .api_route("/api-tokens/:access_token/", patch(update_api_token))
        .api_route("/api-tokens/:access_token/", delete(revoke_api_token))
}
