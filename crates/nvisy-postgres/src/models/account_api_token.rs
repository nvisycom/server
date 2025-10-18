//! Account API token model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::account_api_tokens;
use crate::types::ApiTokenType;

/// Account API token model representing an authentication token.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountApiToken {
    /// Unique token identifier used for authentication
    pub access_seq: Uuid,
    /// Unique refresh token for extending token without re-authentication
    pub refresh_seq: Uuid,
    /// Reference to the account this token belongs to
    pub account_id: Uuid,
    /// Two-character region/state code where token originated
    pub region_code: String,
    /// ISO 3166-1 alpha-2 country code where token originated
    pub country_code: Option<String>,
    /// City name where token originated
    pub city_name: Option<String>,
    /// IP address from which the token was initiated
    pub ip_address: IpNet,
    /// User agent string from the client browser/application
    pub user_agent: String,
    /// Optional persistent device identifier
    pub device_id: Option<String>,
    /// Type of token (web, mobile, api, etc.)
    pub session_type: ApiTokenType,
    /// Flag indicating potentially suspicious token activity
    pub is_suspicious: bool,
    /// Flag indicating if this is a "remember me" extended token
    pub is_remembered: bool,
    /// Timestamp of token creation
    pub issued_at: OffsetDateTime,
    /// Timestamp when the token expires and becomes invalid
    pub expired_at: OffsetDateTime,
    /// Timestamp of most recent token activity
    pub last_used_at: Option<OffsetDateTime>,
    /// Timestamp when the token was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new account API token.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccountApiToken {
    /// Reference to the account this token belongs to
    pub account_id: Uuid,
    /// Two-character region/state code where token originated
    pub region_code: String,
    /// ISO 3166-1 alpha-2 country code where token originated
    pub country_code: Option<String>,
    /// City name where token originated
    pub city_name: Option<String>,
    /// IP address from which the token was initiated
    pub ip_address: IpNet,
    /// User agent string from the client browser/application
    pub user_agent: String,
    /// Optional persistent device identifier
    pub device_id: Option<String>,
    /// Type of token (web, mobile, api, etc.)
    pub session_type: ApiTokenType,
    /// Flag indicating if this is a "remember me" extended token
    pub is_remembered: bool,
    /// Timestamp when the token expires and becomes invalid
    pub expired_at: OffsetDateTime,
}

/// Data for updating an account API token.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = account_api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccountApiToken {
    /// Timestamp of most recent token activity
    pub last_used_at: Option<OffsetDateTime>,
    /// Flag indicating potentially suspicious token activity
    pub is_suspicious: Option<bool>,
    /// Flag indicating if this is a "remember me" extended token
    pub is_remembered: Option<bool>,
    /// Timestamp when the token expires and becomes invalid
    pub expired_at: Option<OffsetDateTime>,
    /// Timestamp when the token was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
}

impl Default for NewAccountApiToken {
    fn default() -> Self {
        Self {
            account_id: Uuid::new_v4(),
            region_code: String::new(),
            country_code: None,
            city_name: None,
            ip_address: "127.0.0.1/32".parse().unwrap(),
            user_agent: String::new(),
            device_id: None,
            session_type: ApiTokenType::Web,
            is_remembered: false,
            expired_at: OffsetDateTime::now_utc() + time::Duration::hours(24), // 24 hour default
        }
    }
}

impl AccountApiToken {
    /// Returns whether the token is currently valid (not expired or deleted).
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_deleted()
    }

    /// Returns whether the token has expired.
    pub fn is_expired(&self) -> bool {
        OffsetDateTime::now_utc() > self.expired_at
    }

    /// Returns whether the token is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the token is flagged as suspicious.
    pub fn is_suspicious(&self) -> bool {
        self.is_suspicious
    }

    /// Returns whether the token is active (valid and recently used).
    pub fn is_active(&self) -> bool {
        self.is_valid() && self.is_recently_used()
    }

    /// Returns whether the token was used recently (within last 30 minutes).
    pub fn is_recently_used(&self) -> bool {
        let now = OffsetDateTime::now_utc();
        if let Some(last_used) = self.last_used_at {
            let duration = now - last_used;
            duration.whole_minutes() < 30
        } else {
            false
        }
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
    pub fn time_until_expiry(&self) -> Option<time::Duration> {
        let now = OffsetDateTime::now_utc();
        if self.expired_at > now {
            Some(self.expired_at - now)
        } else {
            None
        }
    }

    /// Returns the duration since the token was last used.
    pub fn time_since_last_used(&self) -> time::Duration {
        if let Some(last_used) = self.last_used_at {
            OffsetDateTime::now_utc() - last_used
        } else {
            OffsetDateTime::now_utc() - self.issued_at
        }
    }

    /// Returns the total duration the token has been active.
    pub fn token_duration(&self) -> time::Duration {
        if let Some(last_used) = self.last_used_at {
            last_used - self.issued_at
        } else {
            time::Duration::ZERO
        }
    }

    /// Returns whether the token is about to expire (within specified minutes).
    pub fn is_expiring_soon(&self, minutes: i64) -> bool {
        if let Some(remaining) = self.time_until_expiry() {
            remaining.whole_minutes() <= minutes
        } else {
            false
        }
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

    /// Returns whether the token was created recently (within last hour).
    pub fn is_recently_created(&self) -> bool {
        let now = OffsetDateTime::now_utc();
        let duration = now - self.issued_at;
        duration.whole_hours() < 1
    }

    /// Returns whether the token is long-lived (active for more than 24 hours).
    pub fn is_long_lived(&self) -> bool {
        self.token_duration().whole_hours() > 24
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
