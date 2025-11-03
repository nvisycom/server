//! Account action token model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::schema::account_action_tokens;
use crate::types::ActionTokenType;

/// Account action token model representing an action token for account operations.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_action_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountActionToken {
    /// Unique identifier for the token
    pub action_token: Uuid,
    /// Reference to the account this token belongs to
    pub account_id: Uuid,
    /// Type of action this token authorizes
    pub action_type: ActionTokenType,
    /// Additional context data for the token action (JSON, 2B-4KB)
    pub action_data: serde_json::Value,
    /// IP address where the token was generated
    pub ip_address: IpNet,
    /// User agent of the client that generated the token
    pub user_agent: String,
    /// Optional device identifier for additional security tracking
    pub device_id: Option<String>,
    /// Number of times this token has been attempted
    pub attempt_count: i32,
    /// Maximum allowed attempts before token becomes invalid
    pub max_attempts: i32,
    /// Timestamp when the token was created
    pub issued_at: OffsetDateTime,
    /// Timestamp when the token expires
    pub expired_at: OffsetDateTime,
    /// Timestamp when the token was successfully used
    pub used_at: Option<OffsetDateTime>,
}

/// Data for creating a new account action token.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = account_action_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccountActionToken {
    /// Reference to the account this token belongs to
    pub account_id: Uuid,
    /// Type of action this token authorizes
    pub action_type: ActionTokenType,
    /// Additional context data for the token action
    pub action_data: Option<serde_json::Value>,
    /// IP address where the token was generated
    pub ip_address: IpNet,
    /// User agent of the client that generated the token
    pub user_agent: String,
    /// Optional device identifier for additional security tracking
    pub device_id: Option<String>,
    /// Maximum allowed attempts before token becomes invalid
    pub max_attempts: Option<i32>,
    /// Timestamp when the token expires
    pub expired_at: Option<OffsetDateTime>,
}

/// Data for updating an account action token.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = account_action_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccountActionToken {
    /// Number of times this token has been attempted
    pub attempt_count: Option<i32>,
    /// Timestamp when the token was successfully used
    pub used_at: Option<OffsetDateTime>,
}

impl AccountActionToken {
    /// Returns whether the token is still valid (not expired and not used).
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_used()
    }

    /// Returns whether the token has expired.
    pub fn is_expired(&self) -> bool {
        OffsetDateTime::now_utc() > self.expired_at
    }

    /// Returns whether the token has been used.
    pub fn is_used(&self) -> bool {
        self.used_at.is_some()
    }

    /// Returns whether the token can still be used.
    pub fn can_be_used(&self) -> bool {
        self.is_valid() && !self.has_exceeded_attempts()
    }

    /// Returns whether the token has exceeded maximum attempts.
    pub fn has_exceeded_attempts(&self) -> bool {
        self.attempt_count >= self.max_attempts
    }

    /// Returns the remaining time until token expires.
    pub fn time_until_expiry(&self) -> Option<Duration> {
        let now = OffsetDateTime::now_utc();
        if self.expired_at > now {
            Some(self.expired_at - now)
        } else {
            None
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

    /// Returns whether the token was created recently (within last hour).
    pub fn is_recently_created(&self) -> bool {
        let now = OffsetDateTime::now_utc();
        let duration = now - self.issued_at;
        duration.whole_hours() < 1
    }

    /// Returns whether the token was used recently (within last hour).
    pub fn is_recently_used(&self) -> bool {
        if let Some(used_time) = self.used_at {
            let now = OffsetDateTime::now_utc();
            let duration = now - used_time;
            duration.whole_hours() < 1
        } else {
            false
        }
    }

    /// Returns whether this is an account activation token.
    pub fn is_account_activation(&self) -> bool {
        self.action_type == ActionTokenType::ActivateAccount
    }

    /// Returns whether this is a password reset token.
    pub fn is_password_reset(&self) -> bool {
        self.action_type == ActionTokenType::ResetPassword
    }

    /// Returns whether this is an email update token.
    pub fn is_email_update(&self) -> bool {
        self.action_type == ActionTokenType::UpdateEmail
    }

    /// Returns whether this is a two-factor authentication token.
    pub fn is_two_factor_enable(&self) -> bool {
        self.action_type == ActionTokenType::Enable2fa
    }

    /// Returns whether this is a login verification token.
    pub fn is_login_verification(&self) -> bool {
        self.action_type == ActionTokenType::LoginVerification
    }

    /// Returns the remaining attempts before the token is blocked.
    pub fn remaining_attempts(&self) -> i32 {
        (self.max_attempts - self.attempt_count).max(0)
    }

    /// Returns a shortened version of the token for logging/display.
    pub fn token_short(&self) -> String {
        let token_str = self.action_token.to_string();
        if token_str.len() > 8 {
            format!("{}...", &token_str[..8])
        } else {
            token_str
        }
    }

    /// Returns whether the token requires immediate action.
    pub fn requires_immediate_action(&self) -> bool {
        matches!(
            self.action_type,
            ActionTokenType::ResetPassword | ActionTokenType::LoginVerification
        )
    }

    /// Returns the recommended expiry time for this token type.
    pub fn recommended_expiry_duration(&self) -> Duration {
        Duration::seconds(self.action_type.default_expiration_seconds() as i64)
    }

    /// Returns whether the token has context data.
    pub fn has_context_data(&self) -> bool {
        !self
            .action_data
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns the duration since the token was created.
    pub fn age(&self) -> Duration {
        OffsetDateTime::now_utc() - self.issued_at
    }

    /// Returns whether the token is stale (created more than recommended duration ago but not expired).
    pub fn is_stale(&self) -> bool {
        !self.is_expired() && self.age() > self.recommended_expiry_duration()
    }
}
