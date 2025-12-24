//! API token management handlers for user API token operations.
//!
//! This module provides comprehensive API token management functionality including
//! creation, listing, updating, revoking, and statistics. All operations follow
//! security best practices with proper authorization, input validation, and audit logging.

use aide::axum::ApiRouter;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum_extra::headers::UserAgent;
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use jiff::{Span, Timestamp};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewAccountApiToken, UpdateAccountApiToken};
use nvisy_postgres::query::{AccountApiTokenRepository, Pagination as QueryPagination};
use nvisy_postgres::types::ApiTokenType;
use uuid::Uuid;

use super::request::{
    CreateApiToken, ListApiTokensQuery, Pagination, RevokeApiToken, UpdateApiToken,
};
use super::response::{ApiToken, ApiTokenCreated, ApiTokenList, ApiTokenOperation};
use crate::extract::{AuthState, ClientIp, Json, TypedHeader, ValidateJson};
use crate::handler::{ErrorKind, Result};
use crate::service::ServiceState;

/// Tracing target for API token operations.
const TRACING_TARGET: &str = "nvisy_server::handler::api_tokens";

/// Creates a new API token for the authenticated account.
#[tracing::instrument(skip_all)]
async fn create_api_token(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    ClientIp(ip_address): ClientIp,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    ValidateJson(request): ValidateJson<CreateApiToken>,
) -> Result<(StatusCode, Json<ApiTokenCreated>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        name = %request.name,
        has_description = request.description.is_some(),
        has_expiration = request.expires_at.is_some(),
        "creating API token"
    );

    // Sanitize and validate input
    let sanitized_name = request.name.trim().to_string();
    if sanitized_name.is_empty() {
        return Err(ErrorKind::BadRequest
            .with_resource("api_token")
            .with_message("Token name cannot be empty or whitespace only"));
    }

    let sanitized_description = request
        .description
        .as_ref()
        .map(|desc| desc.trim().to_string())
        .filter(|desc| !desc.is_empty());

    // Validate and set expiration date
    let expires_at = match request.expires_at {
        Some(expiry) => {
            let now = Timestamp::now();

            if expiry <= now {
                return Err(ErrorKind::BadRequest
                    .with_resource("api_token")
                    .with_message("Expiration date must be in the future"));
            }

            if expiry > now + Span::new().days(365) {
                return Err(ErrorKind::BadRequest
                    .with_resource("api_token")
                    .with_message("Expiration date cannot exceed 1 year from now"));
            }

            Some(expiry)
        }
        None => None,
    };

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
        name: request.name,
        description: request.description,
        region_code: None,  // Would need geolocation service to populate
        country_code: None, // Would need geolocation service to populate
        city_name: None,    // Would need geolocation service to populate
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
        has_description = sanitized_description.is_some(),
        has_device_id = false,
        "API token created"
    );

    let response = ApiTokenCreated::new(token);
    Ok((StatusCode::CREATED, Json(response)))
}

/// Lists API tokens for the authenticated account.
#[tracing::instrument(skip_all)]
async fn list_api_tokens(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Query(query_params): Query<ListApiTokensQuery>,
    Query(pagination): Query<Pagination>,
) -> Result<(StatusCode, Json<ApiTokenList>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        include_expired = query_params.include_expired,
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

    // For now, we'll use the basic list method and filter in application
    // In a production app, you'd want to add these filters to the database query
    let tokens = if query_params.include_expired.unwrap_or(false) {
        pg_client
            .list_all_account_tokens(auth_claims.account_id, pagination)
            .await?
    } else {
        pg_client
            .list_account_tokens(auth_claims.account_id, pagination)
            .await?
    };

    // Apply additional filters
    let mut filtered_tokens = tokens;

    if let Some(token_type) = query_params.token_type {
        filtered_tokens.retain(|token| token.session_type == token_type);
    }

    if let Some(created_after) = query_params.created_after {
        filtered_tokens.retain(|token| token.issued_at >= created_after.into());
    }

    if let Some(created_before) = query_params.created_before {
        filtered_tokens.retain(|token| token.issued_at <= created_before.into());
    }

    let api_tokens: Vec<ApiToken> = filtered_tokens.into_iter().map(ApiToken::from).collect();
    let total_count = api_tokens.len() as i64;

    let response = ApiTokenList {
        api_tokens,
        total_count,
        page: pagination.offset / pagination.limit,
        page_size: pagination.limit,
        has_more: total_count > pagination.limit,
    };

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        count = response.api_tokens.len(),
        "API tokens listed"
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Gets a specific API token by access token.
#[tracing::instrument(skip_all)]
async fn get_api_token(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(access_token): Path<Uuid>,
) -> Result<(StatusCode, Json<ApiToken>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        access_token = %access_token,
        "getting API token"
    );

    let Some(token) = pg_client.find_token_by_access_token(access_token).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found")
            .with_context(format!("Token ID: {}", access_token)));
    };

    // Ensure the token belongs to the authenticated account
    if token.account_id != auth_claims.account_id {
        return Err(ErrorKind::Forbidden
            .with_resource("api_token")
            .with_message("You do not have permission to access this API token"));
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
) -> Result<(StatusCode, Json<ApiTokenOperation>)> {
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
            .with_message("API token not found")
            .with_context(format!("Token ID: {}", access_token)));
    };

    if existing_token.account_id != auth_claims.account_id {
        return Err(ErrorKind::Forbidden
            .with_resource("api_token")
            .with_message("You do not have permission to modify this API token"));
    }

    let update_token = UpdateAccountApiToken {
        last_used_at: Some(Timestamp::now().into()),
        name: request.name,
        description: request.description,
        ..Default::default()
    };

    pg_client.update_token(access_token, update_token).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        token_preview = %existing_token.access_seq_short(),
        "API token updated"
    );

    Ok((StatusCode::OK, Json(ApiTokenOperation::updated())))
}

/// Revokes (soft deletes) an API token.
#[tracing::instrument(skip_all)]
async fn revoke_api_token(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Path(access_token): Path<Uuid>,
    request: Option<Json<RevokeApiToken>>,
) -> Result<(StatusCode, Json<ApiTokenOperation>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        access_token = %access_token,
        revocation_reason = request.as_ref().and_then(|r| r.reason.as_deref()),
        "revoking API token"
    );

    // First, verify the token exists and belongs to the authenticated account
    let Some(existing_token) = pg_client.find_token_by_access_token(access_token).await? else {
        return Err(ErrorKind::NotFound
            .with_resource("api_token")
            .with_message("API token not found")
            .with_context(format!("Token ID: {}", access_token)));
    };

    if existing_token.account_id != auth_claims.account_id {
        return Err(ErrorKind::Forbidden
            .with_resource("api_token")
            .with_message("You do not have permission to revoke this API token"));
    }

    let deleted = pg_client.delete_token(access_token).await?;

    if !deleted {
        return Err(ErrorKind::BadRequest
            .with_resource("api_token")
            .with_message("API token is already revoked or cannot be revoked"));
    }

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        token_preview = %existing_token.access_seq_short(),
        revocation_reason = request.as_ref().and_then(|r| r.reason.as_deref()),
        "API token revoked"
    );

    Ok((StatusCode::OK, Json(ApiTokenOperation::revoked())))
}

/// Returns a [`Router`] with all related routes.
///
/// [`Router`]: axum::routing::Router
pub fn routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/api-tokens/", post(create_api_token))
        .api_route("/api-tokens/", get(list_api_tokens))
        .api_route("/api-tokens/:access_token", get(get_api_token))
        .api_route("/api-tokens/:access_token", patch(update_api_token))
        .api_route("/api-tokens/:access_token", delete(revoke_api_token))
}

#[cfg(test)]
mod test {
    use crate::handler::test::create_test_server_with_router;
    use crate::handler::tokens::routes;

    #[tokio::test]
    async fn handlers_startup() -> anyhow::Result<()> {
        let server = create_test_server_with_router(|_state| routes()).await?;

        // Creates API token
        let response = server.post("/api-tokens/").await;
        response.assert_status_success();

        // Lists API tokens
        let response = server.get("/api-tokens/").await;
        response.assert_status_success();

        // Gets specific API token
        let response = server
            .get("/api-tokens/123e4567-e89b-12d3-a456-426614174000")
            .await;
        response.assert_status_success();

        // Updates API token
        let response = server
            .patch("/api-tokens/123e4567-e89b-12d3-a456-426614174000")
            .await;
        response.assert_status_success();

        // Revokes specific API token
        let response = server
            .delete("/api-tokens/123e4567-e89b-12d3-a456-426614174000")
            .await;
        response.assert_status_success();

        Ok(())
    }
}
