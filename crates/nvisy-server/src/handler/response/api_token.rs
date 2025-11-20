//! Response structures for API token operations.

use nvisy_postgres::model::AccountApiToken;
use nvisy_postgres::types::ApiTokenType;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// API token response structure.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiToken {
    /// Shortened access token identifier for display.
    #[schema(example = "abcd1234...")]
    pub access_token_preview: String,

    /// Shortened refresh token identifier for display.
    #[schema(example = "efgh5678...")]
    pub refresh_token_preview: String,

    /// Reference to the account this token belongs to.
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub account_id: Uuid,

    /// Human-readable name for the API token.
    #[schema(example = "Production API Token")]
    pub name: String,

    /// Description of the API token.
    #[schema(example = "API token for production deployment automation")]
    pub description: Option<String>,

    /// Type of token (web, mobile, api, etc.).
    #[schema(example = "Api")]
    pub session_type: ApiTokenType,

    /// Whether the token has expired.
    #[schema(example = false)]
    pub is_expired: bool,

    /// Timestamp of token creation.
    #[schema(example = "2024-01-01T00:00:00Z")]
    pub issued_at: OffsetDateTime,

    /// Timestamp when the token expires and becomes invalid.
    #[schema(example = "2024-12-31T23:59:59Z")]
    pub expired_at: OffsetDateTime,

    /// Timestamp of most recent token activity.
    #[schema(example = "2024-06-15T10:30:00Z")]
    pub last_used_at: Option<OffsetDateTime>,
}

/// Response when creating a new API token (includes actual tokens).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiTokenCreated {
    /// The newly created API token details.
    pub api_token: ApiToken,

    /// Full access token (only shown once at creation).
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub access_token: Uuid,

    /// Full refresh token (only shown once at creation).
    #[schema(example = "987fcdeb-51a2-43d1-b456-123456789abc")]
    pub refresh_token: Uuid,
}

/// Paginated list of API tokens.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiTokenList {
    /// List of API tokens.
    pub api_tokens: Vec<ApiToken>,

    /// Total number of API tokens matching the query.
    #[schema(example = 25)]
    pub total_count: i64,

    /// Current page number (0-based).
    #[schema(example = 0)]
    pub page: i64,

    /// Number of items per page.
    #[schema(example = 20)]
    pub page_size: i64,

    /// Whether there are more pages available.
    #[schema(example = true)]
    pub has_more: bool,
}

/// Simple success response for operations like revoke/delete.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiTokenOperation {
    /// Success message.
    #[schema(example = "API token revoked successfully")]
    pub message: String,

    /// Timestamp of the operation.
    #[schema(example = "2024-06-15T10:30:00Z")]
    pub timestamp: OffsetDateTime,
}

impl From<AccountApiToken> for ApiToken {
    fn from(token: AccountApiToken) -> Self {
        Self {
            access_token_preview: token.access_seq_short(),
            refresh_token_preview: token.refresh_seq_short(),
            account_id: token.account_id,
            session_type: token.session_type,
            is_expired: token.is_expired(),
            issued_at: token.issued_at,
            expired_at: token.expired_at,
            last_used_at: token.last_used_at,
            name: token.name,
            description: token.description,
        }
    }
}

impl ApiTokenCreated {
    /// Creates a new response for a created API token.
    pub fn new(token: AccountApiToken) -> Self {
        let access_token = token.access_seq;
        let refresh_token = token.refresh_seq;

        Self {
            api_token: token.into(),
            access_token,
            refresh_token,
        }
    }
}

impl ApiTokenOperation {
    /// Creates a success response with a custom message.
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            timestamp: OffsetDateTime::now_utc(),
        }
    }

    /// Creates a response for successful API token revocation.
    pub fn revoked() -> Self {
        Self::success("API token has been revoked successfully")
    }

    /// Creates a response for successful API token update.
    pub fn updated() -> Self {
        Self::success("API token has been updated successfully")
    }
}
