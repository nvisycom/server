//! Request structures for API token operations.

use nvisy_postgres::types::ApiTokenType;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use super::validation::validation_error;

/// Request to create a new API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
pub struct CreateApiToken {
    /// Human-readable name for the API token.
    #[validate(length(min = 1, max = 100))]
    #[schema(example = "Production API Token")]
    pub name: String,

    /// Optional description for the API token.
    #[validate(length(max = 500))]
    #[schema(example = "API token for production deployment automation")]
    pub description: Option<String>,

    /// Optional expiration date for the API token.
    #[validate(custom(function = "validate_expires_at"))]
    #[schema(example = "2024-12-31T23:59:59Z")]
    pub expires_at: Option<OffsetDateTime>,
}

/// Request to update an existing API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateApiToken {
    /// Updated name for the API token.
    #[validate(length(min = 1, max = 100))]
    #[schema(example = "Updated Production API Token")]
    pub name: Option<String>,

    /// Updated description for the API token.
    #[validate(length(max = 500))]
    #[schema(example = "Updated API token description")]
    pub description: Option<String>,
}

/// Request to revoke (soft delete) an API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
pub struct RevokeApiToken {
    /// Reason for revocation (optional).
    #[validate(length(min = 3, max = 200))]
    #[schema(example = "Token compromised")]
    pub reason: Option<String>,
}

// Validation functions

fn validate_expires_at(expires_at: &OffsetDateTime) -> Result<(), ValidationError> {
    let now = time::OffsetDateTime::now_utc();

    // Check if expiration is in the future
    if *expires_at <= now {
        return Err(validation_error(
            "expiry_in_past",
            "Expiration date must be in the future",
        ));
    }

    // Check if expiration is not too far in the future (max 1 year)
    let max_expiry = now + time::Duration::days(365);
    if *expires_at > max_expiry {
        return Err(validation_error(
            "expiry_too_far",
            "Expiration date cannot be more than 1 year in the future",
        ));
    }

    Ok(())
}

/// Query parameters for listing API tokens.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
pub struct ListApiTokensQuery {
    /// Include expired tokens in the results.
    #[schema(example = false)]
    pub include_expired: Option<bool>,

    /// Filter by token type.
    #[schema(example = "api")]
    pub token_type: Option<ApiTokenType>,

    /// Filter tokens created after this date.
    pub created_after: Option<OffsetDateTime>,

    /// Filter tokens created before this date.
    pub created_before: Option<OffsetDateTime>,

    /// Search in token names and descriptions.
    #[validate(length(min = 1, max = 100))]
    #[schema(example = "production")]
    pub search: Option<String>,
}
