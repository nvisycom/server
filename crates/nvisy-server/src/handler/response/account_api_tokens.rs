//! Response structures for API token operations.

use jiff::Timestamp;
use nvisy_postgres::model::AccountApiToken;
use nvisy_postgres::types::ApiTokenType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// API token response structure.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiToken {
    /// Unique identifier for the token.
    pub id: Uuid,
    /// Human-readable display name for the API token.
    pub display_name: String,
    /// Type of token (web, api, etc.).
    pub session_type: ApiTokenType,
    /// Timestamp of token creation.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires (None = never expires).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired_at: Option<Timestamp>,
    /// Timestamp of most recent token activity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<Timestamp>,
    /// Whether this token is the one the current request authenticated with.
    ///
    /// Lets a client single out the active session in the list. Defaults to
    /// `false`; the listing handler sets it for the matching token.
    pub current: bool,
}

impl ApiToken {
    pub fn from_model(token: AccountApiToken) -> Self {
        Self {
            id: token.id,
            display_name: token.display_name,
            session_type: token.session_type,
            issued_at: token.issued_at.into(),
            expired_at: token.expired_at.map(Into::into),
            last_used_at: token.last_used_at.map(Into::into),
            current: false,
        }
    }

    /// Marks whether this token is the current request's session token.
    #[must_use]
    pub fn with_current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }
}

impl ApiToken {
    /// Creates an `ApiTokenWithJWT` by adding a JWT token string.
    pub fn with_jwt(self, jwt: String) -> ApiTokenWithJWT {
        ApiTokenWithJWT {
            id: self.id,
            display_name: self.display_name,
            session_type: self.session_type,
            issued_at: self.issued_at,
            expired_at: self.expired_at,
            token: jwt,
        }
    }
}

/// Paginated response for API tokens.
pub type ApiTokensPage = Page<ApiToken>;

/// API token with JWT token string (only returned on creation).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiTokenWithJWT {
    /// Unique identifier for the token.
    pub id: Uuid,
    /// Human-readable display name for the API token.
    pub display_name: String,
    /// Type of token (web, mobile, api, etc.).
    pub session_type: ApiTokenType,
    /// Timestamp of token creation.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires (omitted = never expires).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired_at: Option<Timestamp>,
    /// The JWT token string (only shown once on creation).
    pub token: String,
}
