//! Response structures for API token operations.

use jiff::Timestamp;
use nvisy_postgres::model::AccountApiToken;
use nvisy_postgres::types::ApiTokenType;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use uuid::Uuid;

/// API token response structure.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApiToken {
    /// Shortened access token identifier for display.
    pub access_token_preview: String,

    /// Shortened refresh token identifier for display.
    pub refresh_token_preview: String,

    /// Reference to the account this token belongs to.
    pub account_id: Uuid,

    /// Human-readable name for the API token.
    pub name: String,

    /// Description of the API token.
    pub description: Option<String>,

    /// Type of token (web, mobile, api, etc.).
    pub session_type: ApiTokenType,

    /// Whether the token has expired.
    pub is_expired: bool,

    /// Timestamp of token creation.
    pub issued_at: Timestamp,

    /// Timestamp when the token expires and becomes invalid.
    pub expired_at: Timestamp,

    /// Timestamp of most recent token activity.
    pub last_used_at: Option<Timestamp>,
}

/// Response when creating a new API token (includes actual tokens).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApiTokenCreated {
    /// The newly created API token details.
    pub api_token: ApiToken,

    /// Full access token (only shown once at creation).
    pub access_token: Uuid,

    /// Full refresh token (only shown once at creation).
    pub refresh_token: Uuid,
}

/// Paginated list of API tokens.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApiTokenList {
    /// List of API tokens.
    pub api_tokens: Vec<ApiToken>,

    /// Total number of API tokens matching the query.
    pub total_count: i64,

    /// Current page number (0-based).
    pub page: i64,

    /// Number of items per page.
    pub page_size: i64,

    /// Whether there are more pages available.
    pub has_more: bool,
}

/// Simple success response for operations like revoke/delete.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApiTokenOperation {
    /// Success message.
    pub message: String,

    /// Timestamp of the operation.
    pub timestamp: Timestamp,
}

impl From<AccountApiToken> for ApiToken {
    fn from(token: AccountApiToken) -> Self {
        Self {
            access_token_preview: token.access_seq_short(),
            refresh_token_preview: token.refresh_seq_short(),
            account_id: token.account_id,
            session_type: token.session_type,
            is_expired: token.is_expired(),
            issued_at: token.issued_at.into(),
            expired_at: token.expired_at.into(),
            last_used_at: token.last_used_at.map(Into::into),
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
            timestamp: Timestamp::now(),
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
