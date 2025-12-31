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
    /// Unique identifier for the token.
    pub id: Uuid,

    /// Reference to the account this token belongs to.
    pub account_id: Uuid,

    /// Human-readable name for the API token.
    pub name: String,

    /// Type of token (web, mobile, api, etc.).
    pub session_type: ApiTokenType,

    /// Whether the token has expired.
    pub is_expired: bool,

    /// Timestamp of token creation.
    pub issued_at: Timestamp,

    /// Timestamp when the token expires (None = never expires).
    pub expired_at: Option<Timestamp>,

    /// Timestamp of most recent token activity.
    pub last_used_at: Option<Timestamp>,
}

impl ApiToken {
    /// Creates an ApiToken response from a database model.
    pub fn from_model(token: AccountApiToken) -> Self {
        let is_expired = token.is_expired();
        Self {
            id: token.id,
            account_id: token.account_id,
            name: token.name,
            session_type: token.session_type,
            is_expired,
            issued_at: token.issued_at.into(),
            expired_at: token.expired_at.map(Into::into),
            last_used_at: token.last_used_at.map(Into::into),
        }
    }

    /// Creates a list of ApiToken responses from database models.
    pub fn from_models(models: Vec<AccountApiToken>) -> Vec<Self> {
        models.into_iter().map(Self::from_model).collect()
    }

    /// Creates an `ApiTokenWithJWT` by adding a JWT token string.
    pub fn with_jwt(self, jwt: String) -> ApiTokenWithJWT {
        ApiTokenWithJWT {
            id: self.id,
            account_id: self.account_id,
            name: self.name,
            session_type: self.session_type,
            issued_at: self.issued_at,
            expired_at: self.expired_at,
            token: jwt,
        }
    }
}

/// Response for listing API tokens.
pub type ApiTokens = Vec<ApiToken>;

/// API token with JWT token string (only returned on creation).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiTokenWithJWT {
    /// Unique identifier for the token.
    pub id: Uuid,

    /// Reference to the account this token belongs to.
    pub account_id: Uuid,

    /// Human-readable name for the API token.
    pub name: String,

    /// Type of token (web, mobile, api, etc.).
    pub session_type: ApiTokenType,

    /// Timestamp of token creation.
    pub issued_at: Timestamp,

    /// Timestamp when the token expires (None = never expires).
    pub expired_at: Option<Timestamp>,

    /// The JWT token string (only shown once on creation).
    pub token: String,
}
