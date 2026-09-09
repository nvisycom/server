//! Account API token model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::account_api_tokens;
use crate::types::ApiTokenType;

/// Account API token model representing an authentication token.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct AccountApiToken {
    /// Unique identifier for the token.
    pub id: Uuid,
    /// Reference to the account this token belongs to.
    pub account_id: Uuid,
    /// Human-readable display name for the API token.
    pub display_name: String,
    /// Type of token (web, mobile, api, etc.).
    pub session_type: ApiTokenType,
    /// IP address from which the token was initiated.
    pub ip_address: Option<IpNet>,
    /// User agent string from the client browser/application.
    pub user_agent: Option<String>,
    /// Flag indicating if this is a "remember me" extended token.
    pub is_remembered: bool,
    /// Timestamp of token creation.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires and becomes invalid (None = never expires).
    pub expired_at: Option<Timestamp>,
    /// Timestamp of most recent token activity.
    pub last_used_at: Option<Timestamp>,
    /// Timestamp when the token was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new account API token.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewAccountApiToken {
    /// Reference to the account this token belongs to.
    pub account_id: Uuid,
    /// Human-readable display name for the API token.
    pub display_name: String,
    /// Type of token (web, mobile, api, etc.).
    pub session_type: Option<ApiTokenType>,
    /// IP address from which the token was initiated.
    pub ip_address: Option<IpNet>,
    /// User agent string from the client browser/application.
    pub user_agent: Option<String>,
    /// Flag indicating if this is a "remember me" extended token.
    pub is_remembered: Option<bool>,
    /// Timestamp when the token expires and becomes invalid.
    pub expired_at: Option<Timestamp>,
}

impl NewAccountApiToken {
    /// A minimal token of `session_type` for `account_id`, for tests. No expiry,
    /// no remembered flag, no client metadata.
    #[cfg(any(feature = "test_util", test))]
    pub fn test(account_id: Uuid, session_type: ApiTokenType) -> Self {
        Self {
            account_id,
            display_name: "Test Token".to_owned(),
            session_type: Some(session_type),
            ..Default::default()
        }
    }
}

/// Data for updating an account API token.
#[derive(Debug, Default, Clone, AsChangeset)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateAccountApiToken {
    /// Timestamp of token creation (the absolute-cap anchor).
    pub issued_at: Option<Timestamp>,
    /// Timestamp of most recent token activity.
    pub last_used_at: Option<Option<Timestamp>>,
    /// Updated display name for the API token.
    pub display_name: Option<String>,
    /// Flag indicating if this is a "remember me" extended token.
    pub is_remembered: Option<bool>,
    /// Timestamp when the token expires and becomes invalid.
    pub expired_at: Option<Option<Timestamp>>,
    /// Timestamp when the token was soft-deleted.
    pub deleted_at: Option<Option<Timestamp>>,
}
