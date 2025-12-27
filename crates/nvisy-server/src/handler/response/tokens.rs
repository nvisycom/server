//! Response structures for API token operations.

use jiff::Timestamp;
use nvisy_postgres::model::AccountApiToken;
use nvisy_postgres::types::ApiTokenType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// API token response structure.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
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

/// Response when creating a new API token (includes actual tokens, shown only once).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiTokenWithSecret {
    /// Base API token information.
    #[serde(flatten)]
    pub api_token: ApiToken,

    /// Full access token (only shown once at creation).
    pub access_token: Uuid,

    /// Full refresh token (only shown once at creation).
    pub refresh_token: Uuid,
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

impl ApiTokenWithSecret {
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

impl From<AccountApiToken> for ApiTokenWithSecret {
    #[inline]
    fn from(token: AccountApiToken) -> Self {
        Self::new(token)
    }
}

/// Response for listing API tokens.
pub type ApiTokens = Vec<ApiToken>;
