//! API token management handlers for user API token operations.
//!
//! This module provides comprehensive API token management functionality including
//! creation, listing, updating, revoking, and statistics. All operations follow
//! security best practices with proper authorization, input validation, and audit logging.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum_client_ip::ClientIp;
use axum_extra::TypedHeader;
use axum_extra::headers::UserAgent;
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use nvisy_postgres::PgClient;
use nvisy_postgres::model::{NewAccountApiToken, UpdateAccountApiToken};
use nvisy_postgres::query::{AccountApiTokenRepository, Pagination};
use nvisy_postgres::types::ApiTokenType;
use time::OffsetDateTime;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::request::{CreateApiToken, ListApiTokensQuery, RevokeApiToken, UpdateApiToken};
use super::response::{ApiToken, ApiTokenCreated, ApiTokenList, ApiTokenOperation};
use crate::extract::{AuthState, Json, ValidateJson};
use crate::handler::{ErrorKind, ErrorResponse, PaginationRequest, Result};
use crate::service::ServiceState;

/// Tracing target for API token operations.
const TRACING_TARGET: &str = "nvisy_server::handler::api_tokens";

/// Default API token expiration duration (365 days).
const DEFAULT_TOKEN_EXPIRATION_DAYS: i64 = 365;

/// Maximum allowed API token expiration duration (2 years).
const MAX_TOKEN_EXPIRATION_DAYS: i64 = 730;

/// Maximum length for API token names.
const MAX_TOKEN_NAME_LENGTH: usize = 100;

/// Maximum length for API token descriptions.
const MAX_TOKEN_DESCRIPTION_LENGTH: usize = 500;

/// Maximum length for device IDs.
const MAX_DEVICE_ID_LENGTH: usize = 100;

/// Creates a new API token for the authenticated account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    post, path = "/api-tokens/", tag = "api-tokens",
    request_body = CreateApiToken,
    responses(
        (
            status = CREATED,
            description = "API token created successfully",
            body = ApiTokenCreated,
        ),
        (
            status = BAD_REQUEST,
            description = "Invalid request data",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
    ),
)]
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

    let mut conn = pg_client.get_connection().await?;

    // Sanitize and validate input
    let sanitized_name = request.name.trim().to_string();
    if sanitized_name.is_empty() {
        return Err(ErrorKind::BadRequest
            .with_resource("api_token")
            .with_message("Token name cannot be empty or whitespace only"));
    }

    if sanitized_name.len() > MAX_TOKEN_NAME_LENGTH {
        return Err(ErrorKind::BadRequest
            .with_resource("api_token")
            .with_message(format!(
                "Token name cannot exceed {} characters",
                MAX_TOKEN_NAME_LENGTH
            )));
    }

    let sanitized_description = request
        .description
        .as_ref()
        .map(|desc| desc.trim().to_string())
        .filter(|desc| !desc.is_empty());

    if let Some(ref desc) = sanitized_description {
        if desc.len() > MAX_TOKEN_DESCRIPTION_LENGTH {
            return Err(ErrorKind::BadRequest
                .with_resource("api_token")
                .with_message(format!(
                    "Token description cannot exceed {} characters",
                    MAX_TOKEN_DESCRIPTION_LENGTH
                )));
        }
    }

    let sanitized_device_id = request
        .device_id
        .as_ref()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());

    if let Some(ref device_id) = sanitized_device_id {
        if device_id.len() > MAX_DEVICE_ID_LENGTH {
            return Err(ErrorKind::BadRequest
                .with_resource("api_token")
                .with_message(format!(
                    "Device ID cannot exceed {} characters",
                    MAX_DEVICE_ID_LENGTH
                )));
        }
    }

    // Validate and set expiration date
    let expires_at = match request.expires_at {
        Some(expiry) => {
            let now = OffsetDateTime::now_utc();
            let max_expiry = now + time::Duration::days(MAX_TOKEN_EXPIRATION_DAYS);

            if expiry <= now {
                return Err(ErrorKind::BadRequest
                    .with_resource("api_token")
                    .with_message("Expiration date must be in the future"));
            }

            if expiry > max_expiry {
                return Err(ErrorKind::BadRequest
                    .with_resource("api_token")
                    .with_message("Expiration date cannot exceed 2 years from now"));
            }

            expiry
        }
        None => OffsetDateTime::now_utc() + time::Duration::days(DEFAULT_TOKEN_EXPIRATION_DAYS),
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
        device_id: sanitized_device_id.clone(),
        session_type: Some(ApiTokenType::Api),
        is_remembered: Some(true),
        expired_at: Some(expires_at),
    };

    let token = AccountApiTokenRepository::create_token(&mut conn, new_token).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        token_preview = %token.access_seq_short(),
        name = %sanitized_name,
        expires_at = %expires_at,
        has_description = sanitized_description.is_some(),
        has_device_id = sanitized_device_id.is_some(),
        "API token created"
    );

    let response = ApiTokenCreated::new(token);
    Ok((StatusCode::CREATED, Json(response)))
}

/// Lists API tokens for the authenticated account.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    get, path = "/api-tokens/", tag = "api-tokens",
    params(
        ("include_expired" = Option<bool>, Query, description = "Include expired tokens"),
        ("token_type" = Option<ApiTokenType>, Query, description = "Filter by token type"),
        ("is_suspicious" = Option<bool>, Query, description = "Filter by suspicious status"),
        ("created_after" = Option<OffsetDateTime>, Query, description = "Filter tokens created after date"),
        ("created_before" = Option<OffsetDateTime>, Query, description = "Filter tokens created before date"),
        ("search" = Option<String>, Query, description = "Search in token names and descriptions"),
        ("page" = Option<i64>, Query, description = "Page number (0-based)"),
        ("limit" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (
            status = OK,
            description = "List of API tokens",
            body = ApiTokenList,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
    ),
)]
async fn list_api_tokens(
    State(pg_client): State<PgClient>,
    AuthState(auth_claims): AuthState,
    Query(query_params): Query<ListApiTokensQuery>,
    Query(pagination): Query<PaginationRequest>,
) -> Result<(StatusCode, Json<ApiTokenList>)> {
    tracing::trace!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        include_expired = query_params.include_expired,
        "listing API tokens"
    );

    let mut conn = pg_client.get_connection().await?;

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

    let pagination = Pagination::from(pagination);

    // For now, we'll use the basic list method and filter in application
    // In a production app, you'd want to add these filters to the database query
    let tokens = if query_params.include_expired.unwrap_or(false) {
        AccountApiTokenRepository::list_all_account_tokens(
            &mut conn,
            auth_claims.account_id,
            pagination,
        )
        .await?
    } else {
        AccountApiTokenRepository::list_account_tokens(
            &mut conn,
            auth_claims.account_id,
            pagination,
        )
        .await?
    };

    // Apply additional filters
    let mut filtered_tokens = tokens;

    if let Some(token_type) = query_params.token_type {
        filtered_tokens.retain(|token| token.session_type == token_type);
    }

    if let Some(is_suspicious) = query_params.is_suspicious {
        filtered_tokens.retain(|token| token.is_suspicious == is_suspicious);
    }

    if let Some(created_after) = query_params.created_after {
        filtered_tokens.retain(|token| token.issued_at >= created_after);
    }

    if let Some(created_before) = query_params.created_before {
        filtered_tokens.retain(|token| token.issued_at <= created_before);
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
#[utoipa::path(
    get, path = "/api-tokens/{access_token}", tag = "api-tokens",
    params(
        ("access_token" = Uuid, Path, description = "Access token UUID")
    ),
    responses(
        (
            status = OK,
            description = "API token details",
            body = ApiToken,
        ),
        (
            status = NOT_FOUND,
            description = "API token not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
    ),
)]
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

    let mut conn = pg_client.get_connection().await?;

    let Some(token) =
        AccountApiTokenRepository::find_token_by_access_token(&mut conn, access_token).await?
    else {
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
#[utoipa::path(
    patch, path = "/api-tokens/{access_token}", tag = "api-tokens",
    params(
        ("access_token" = Uuid, Path, description = "Access token UUID")
    ),
    request_body = UpdateApiToken,
    responses(
        (
            status = OK,
            description = "API token updated successfully",
            body = ApiTokenOperation,
        ),
        (
            status = NOT_FOUND,
            description = "API token not found",
            body = ErrorResponse,
        ),
        (
            status = BAD_REQUEST,
            description = "Invalid request data",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
    ),
)]
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

    let mut conn = pg_client.get_connection().await?;

    // First, verify the token exists and belongs to the authenticated account
    let Some(existing_token) =
        AccountApiTokenRepository::find_token_by_access_token(&mut conn, access_token).await?
    else {
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

    // Validate expiration date if provided
    if let Some(expires_at) = request.expires_at {
        let now = OffsetDateTime::now_utc();
        let max_expiry = now + time::Duration::days(MAX_TOKEN_EXPIRATION_DAYS);

        if expires_at <= now {
            return Err(ErrorKind::BadRequest
                .with_resource("api_token")
                .with_message("Expiration date must be in the future"));
        }

        if expires_at > max_expiry {
            return Err(ErrorKind::BadRequest
                .with_resource("api_token")
                .with_message("Expiration date cannot exceed 2 years from now"));
        }
    }

    let update_token = UpdateAccountApiToken {
        last_used_at: Some(OffsetDateTime::now_utc()),
        name: request.name,
        description: request.description,
        is_suspicious: request.is_suspicious,
        expired_at: request.expires_at,
        ..Default::default()
    };

    AccountApiTokenRepository::update_token(&mut conn, access_token, update_token).await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %auth_claims.account_id,
        token_preview = %existing_token.access_seq_short(),
        updated_expiration = request.expires_at.is_some(),
        marked_suspicious = request.is_suspicious.unwrap_or(false),
        "API token updated"
    );

    Ok((StatusCode::OK, Json(ApiTokenOperation::updated())))
}

/// Revokes (soft deletes) an API token.
#[tracing::instrument(skip_all)]
#[utoipa::path(
    delete, path = "/api-tokens/{access_token}", tag = "api-tokens",
    params(
        ("access_token" = Uuid, Path, description = "Access token UUID")
    ),
    request_body = Option<RevokeApiToken>,
    responses(
        (
            status = OK,
            description = "API token revoked successfully",
            body = ApiTokenOperation,
        ),
        (
            status = NOT_FOUND,
            description = "API token not found",
            body = ErrorResponse,
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            description = "Internal server error",
            body = ErrorResponse,
        ),
    ),
)]
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

    let mut conn = pg_client.get_connection().await?;

    // First, verify the token exists and belongs to the authenticated account
    let Some(existing_token) =
        AccountApiTokenRepository::find_token_by_access_token(&mut conn, access_token).await?
    else {
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

    let deleted = AccountApiTokenRepository::delete_token(&mut conn, access_token).await?;

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
pub fn routes() -> OpenApiRouter<ServiceState> {
    OpenApiRouter::new().routes(routes!(
        create_api_token,
        list_api_tokens,
        get_api_token,
        update_api_token,
        revoke_api_token
    ))
}

#[cfg(test)]
mod test {
    use crate::handler::api_tokens::routes;
    use crate::handler::test::create_test_server_with_router;

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
