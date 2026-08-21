//! Account API token model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::account_api_tokens;
use crate::types::{ApiTokenType, HasCreatedAt, HasExpiresAt, HasSecurityContext};

/// Account API token model representing an authentication token.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
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

/// Data for updating an account API token.
#[derive(Debug, Default, Clone, AsChangeset)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccountApiToken {
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

impl HasCreatedAt for AccountApiToken {
    fn created_at(&self) -> jiff::Timestamp {
        self.issued_at.into()
    }
}

impl HasExpiresAt for AccountApiToken {
    fn expires_at(&self) -> Option<jiff::Timestamp> {
        self.expired_at.map(Into::into)
    }
}

impl HasSecurityContext for AccountApiToken {
    fn ip_address(&self) -> Option<IpNet> {
        self.ip_address
    }

    fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }
}
