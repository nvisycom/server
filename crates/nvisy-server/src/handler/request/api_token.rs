//! Request structures for API token operations.

use nvisy_postgres::types::ApiTokenType;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use validator::Validate;

/// Request to create a new API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
pub struct CreateApiToken {
    /// Human-readable name for the API token.
    #[validate(length(
        min = 1,
        max = 100,
        message = "Token name must be between 1 and 100 characters"
    ))]
    #[schema(example = "Production API Token")]
    pub name: String,

    /// Optional description for the API token.
    #[validate(length(max = 500, message = "Token description cannot exceed 500 characters"))]
    #[schema(example = "API token for production deployment automation")]
    pub description: Option<String>,

    /// Optional expiration date for the API token.
    #[schema(example = "2024-12-31T23:59:59Z")]
    pub expires_at: Option<OffsetDateTime>,

    /// Optional device identifier.
    #[validate(length(max = 100, message = "Device ID cannot exceed 100 characters"))]
    #[schema(example = "production-server-01")]
    pub device_id: Option<String>,
}

/// Request to update an existing API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateApiToken {
    /// Updated name for the API token.
    #[validate(length(
        min = 1,
        max = 100,
        message = "Token name must be between 1 and 100 characters"
    ))]
    #[schema(example = "Updated Production API Token")]
    pub name: Option<String>,

    /// Updated description for the API token.
    #[validate(length(max = 500, message = "Token description cannot exceed 500 characters"))]
    #[schema(example = "Updated API token description")]
    pub description: Option<String>,

    /// Updated expiration date for the API token.
    #[schema(example = "2025-12-31T23:59:59Z")]
    pub expires_at: Option<OffsetDateTime>,

    /// Mark the token as suspicious.
    #[schema(example = false)]
    pub is_suspicious: Option<bool>,
}

/// Request to revoke (soft delete) an API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
pub struct RevokeApiToken {
    /// Reason for revocation (optional).
    #[validate(length(max = 200, message = "Revocation reason cannot exceed 200 characters"))]
    #[schema(example = "Token compromised")]
    pub reason: Option<String>,
}

/// Query parameters for listing API tokens.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
pub struct ListApiTokensQuery {
    /// Include expired tokens in the results.
    #[schema(example = false)]
    pub include_expired: Option<bool>,

    /// Filter by token type.
    #[schema(example = "Api")]
    pub token_type: Option<ApiTokenType>,

    /// Filter by suspicious status.
    #[schema(example = false)]
    pub is_suspicious: Option<bool>,

    /// Filter tokens created after this date.
    pub created_after: Option<OffsetDateTime>,

    /// Filter tokens created before this date.
    pub created_before: Option<OffsetDateTime>,

    /// Search in token names and descriptions.
    #[validate(length(
        min = 1,
        max = 100,
        message = "Search query must be between 1 and 100 characters"
    ))]
    #[schema(example = "production")]
    pub search: Option<String>,
}
