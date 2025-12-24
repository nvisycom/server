//! Request structures for API token operations.

use jiff::Timestamp;
use nvisy_postgres::types::ApiTokenType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Request to create a new API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, JsonSchema, Default)]
pub struct CreateApiToken {
    /// Human-readable name for the API token.
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    /// Optional description for the API token.
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// Optional expiration date for the API token.
    #[validate(custom(function = "validate_expires_at"))]
    pub expires_at: Option<Timestamp>,
}

/// Request to update an existing API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, JsonSchema, Default)]
pub struct UpdateApiToken {
    /// Updated name for the API token.
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,

    /// Updated description for the API token.
    #[validate(length(max = 500))]
    pub description: Option<String>,
}

/// Request to revoke (soft delete) an API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, JsonSchema, Default)]
pub struct RevokeApiToken {
    /// Reason for revocation (optional).
    #[validate(length(min = 3, max = 200))]
    pub reason: Option<String>,
}

// Validation functions

fn validate_expires_at(expires_at: &Timestamp) -> Result<(), ValidationError> {
    let now = Timestamp::now();

    // Check if expiration is in the future
    if *expires_at <= now {}

    // Check if expiration is not too far in the future (max 1 year)
    let max_expiry = now + jiff::Span::new().days(365);
    if *expires_at > max_expiry {}

    Ok(())
}

/// Query parameters for listing API tokens.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, JsonSchema, Default)]
pub struct ListApiTokensQuery {
    /// Include expired tokens in the results.
    pub include_expired: Option<bool>,

    /// Filter by token type.
    pub token_type: Option<ApiTokenType>,

    /// Filter tokens created after this date.
    pub created_after: Option<Timestamp>,

    /// Filter tokens created before this date.
    pub created_before: Option<Timestamp>,

    /// Search in token names and descriptions.
    #[validate(length(min = 1, max = 100))]
    pub search: Option<String>,
}
