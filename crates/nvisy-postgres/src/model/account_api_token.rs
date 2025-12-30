//! Account API token model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::account_api_tokens;
use crate::types::constants::token;
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
    /// Human-readable name for the API token.
    pub name: String,
    /// Type of token (web, mobile, api, etc.).
    pub session_type: ApiTokenType,
    /// IP address from which the token was initiated.
    pub ip_address: IpNet,
    /// User agent string from the client browser/application.
    pub user_agent: String,
    /// Flag indicating if this is a "remember me" extended token.
    pub is_remembered: bool,
    /// Timestamp of token creation.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires and becomes invalid.
    pub expired_at: Timestamp,
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
    /// Human-readable name for the API token.
    pub name: String,
    /// Type of token (web, mobile, api, etc.).
    pub session_type: Option<ApiTokenType>,
    /// IP address from which the token was initiated.
    pub ip_address: IpNet,
    /// User agent string from the client browser/application.
    pub user_agent: String,
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
    pub last_used_at: Option<Timestamp>,
    /// Updated name for the API token.
    pub name: Option<String>,
    /// Flag indicating if this is a "remember me" extended token.
    pub is_remembered: Option<bool>,
    /// Timestamp when the token expires and becomes invalid.
    pub expired_at: Option<Timestamp>,
    /// Timestamp when the token was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

impl AccountApiToken {
    /// Returns whether the token is currently valid (not expired or deleted).
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_deleted()
    }

    /// Returns whether the token has expired.
    pub fn is_expired(&self) -> bool {
        jiff::Timestamp::now() > jiff::Timestamp::from(self.expired_at)
    }

    /// Returns whether the token is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns the remaining time until token expires.
    pub fn time_until_expiry(&self) -> Option<jiff::Span> {
        let now = jiff::Timestamp::now();
        let expired_at = jiff::Timestamp::from(self.expired_at);
        if expired_at > now {
            Some(expired_at - now)
        } else {
            None
        }
    }

    /// Returns the duration since the token was last used.
    pub fn time_since_last_used(&self) -> jiff::Span {
        let now = jiff::Timestamp::now();
        if let Some(last_used) = self.last_used_at {
            now - jiff::Timestamp::from(last_used)
        } else {
            now - jiff::Timestamp::from(self.issued_at)
        }
    }

    /// Returns the total duration the token has been active.
    pub fn token_duration(&self) -> jiff::Span {
        if let Some(last_used) = self.last_used_at {
            jiff::Timestamp::from(last_used) - jiff::Timestamp::from(self.issued_at)
        } else {
            jiff::Span::new()
        }
    }

    /// Returns whether the token is about to expire (within specified minutes).
    pub fn is_expiring_soon(&self, minutes: i64) -> bool {
        if let Some(remaining) = self.time_until_expiry() {
            remaining.get_minutes() <= minutes
        } else {
            false
        }
    }

    /// Returns whether the token is about to expire (within warning threshold).
    pub fn is_expiring_soon_default(&self) -> bool {
        self.is_expiring_soon(token::EXPIRY_WARNING_MINUTES)
    }

    /// Returns whether this is a web token.
    pub fn is_web_token(&self) -> bool {
        self.session_type == ApiTokenType::Web
    }

    /// Returns whether this is a mobile token.
    pub fn is_mobile_token(&self) -> bool {
        self.session_type == ApiTokenType::Mobile
    }

    /// Returns whether this is an API token.
    pub fn is_api_token(&self) -> bool {
        self.session_type == ApiTokenType::Api
    }

    /// Returns whether the token is long-lived (active for more than 24 hours).
    pub fn is_long_lived(&self) -> bool {
        i64::from(self.token_duration().get_hours()) > token::LONG_LIVED_THRESHOLD_HOURS
    }

    /// Returns a shortened version of the token ID for logging/display.
    pub fn id_short(&self) -> String {
        let id_str = self.id.to_string();
        if id_str.len() > 8 {
            format!("{}...", &id_str[..8])
        } else {
            id_str
        }
    }
}

impl HasCreatedAt for AccountApiToken {
    fn created_at(&self) -> jiff::Timestamp {
        self.issued_at.into()
    }
}

impl HasExpiresAt for AccountApiToken {
    fn expires_at(&self) -> jiff::Timestamp {
        self.expired_at.into()
    }
}

impl HasSecurityContext for AccountApiToken {
    fn ip_address(&self) -> Option<IpNet> {
        Some(self.ip_address)
    }

    fn user_agent(&self) -> Option<&str> {
        Some(&self.user_agent)
    }
}
