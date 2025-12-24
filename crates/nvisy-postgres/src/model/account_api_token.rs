//! Account API token model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::account_api_tokens;
use crate::types::constants::token;
use crate::types::{
    ApiTokenType, HasCreatedAt, HasExpiresAt, HasGeographicContext, HasSecurityContext,
};

/// Account API token model representing an authentication token.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountApiToken {
    /// Unique access token for API authentication.
    pub access_seq: Uuid,
    /// Unique refresh token for extending token without re-authentication.
    pub refresh_seq: Uuid,
    /// Reference to the account this token belongs to.
    pub account_id: Uuid,
    /// Human-readable name for the API token.
    pub name: String,
    /// Optional description for the API token.
    pub description: Option<String>,
    /// Two-character region/state code where token originated.
    pub region_code: String,
    /// ISO 3166-1 alpha-2 country code where token originated.
    pub country_code: Option<String>,
    /// City name where token originated.
    pub city_name: Option<String>,
    /// IP address from which the token was initiated.
    pub ip_address: IpNet,
    /// User agent string from the client browser/application.
    pub user_agent: String,
    /// Optional persistent device identifier.
    pub device_id: Option<String>,
    /// Type of token (web, mobile, api, etc.).
    pub session_type: ApiTokenType,
    /// Flag indicating potentially suspicious token activity.
    pub is_suspicious: bool,
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
    /// Optional description for the API token.
    pub description: Option<String>,
    /// Two-character region/state code where token originated.
    pub region_code: Option<String>,
    /// ISO 3166-1 alpha-2 country code where token originated.
    pub country_code: Option<String>,
    /// City name where token originated.
    pub city_name: Option<String>,
    /// IP address from which the token was initiated.
    pub ip_address: IpNet,
    /// User agent string from the client browser/application.
    pub user_agent: String,
    /// Optional persistent device identifier.
    pub device_id: Option<String>,
    /// Type of token (web, mobile, api, etc.).
    pub session_type: Option<ApiTokenType>,
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
    /// Updated description for the API token.
    pub description: Option<String>,
    /// Flag indicating potentially suspicious token activity.
    pub is_suspicious: Option<bool>,
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

    /// Returns whether the token is flagged as suspicious.
    pub fn is_suspicious(&self) -> bool {
        self.is_suspicious
    }

    /// Returns whether the token can be refreshed.
    pub fn can_be_refreshed(&self) -> bool {
        self.is_valid() && !self.is_suspicious()
    }

    /// Returns whether the token can be extended.
    pub fn can_be_extended(&self) -> bool {
        self.is_valid() && !self.is_suspicious()
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

    /// Returns whether the token has location information.
    pub fn has_location_info(&self) -> bool {
        self.country_code.is_some() || self.city_name.is_some()
    }

    /// Returns a formatted location string for display.
    pub fn location_display(&self) -> String {
        match (&self.city_name, &self.country_code) {
            (Some(city), Some(country)) => format!("{}, {}", city, country),
            (Some(city), None) => city.clone(),
            (None, Some(country)) => country.clone(),
            (None, None) => self.region_code.clone(),
        }
    }

    /// Returns whether the token is long-lived (active for more than 24 hours).
    pub fn is_long_lived(&self) -> bool {
        i64::from(self.token_duration().get_hours()) > token::LONG_LIVED_THRESHOLD_HOURS
    }

    /// Returns a shortened version of the access token for logging/display.
    pub fn access_seq_short(&self) -> String {
        let token_str = self.access_seq.to_string();
        if token_str.len() > 8 {
            format!("{}...", &token_str[..8])
        } else {
            token_str
        }
    }

    /// Returns a shortened version of the refresh token for logging/display.
    pub fn refresh_seq_short(&self) -> String {
        let token_str = self.refresh_seq.to_string();
        if token_str.len() > 8 {
            format!("{}...", &token_str[..8])
        } else {
            token_str
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

impl HasGeographicContext for AccountApiToken {
    fn country_code(&self) -> Option<&str> {
        self.country_code.as_deref()
    }

    fn region_code(&self) -> Option<&str> {
        Some(&self.region_code)
    }

    fn city_name(&self) -> Option<&str> {
        self.city_name.as_deref()
    }
}
