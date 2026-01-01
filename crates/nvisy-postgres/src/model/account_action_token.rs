//! Account action token model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::account_action_tokens;
use crate::types::{ActionTokenType, HasCreatedAt, HasExpiresAt, HasSecurityContext};

/// Account action token model representing a temporary authorization token.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_action_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountActionToken {
    /// Unique identifier for the token.
    pub action_token: Uuid,
    /// Reference to the account this token belongs to.
    pub account_id: Uuid,
    /// Type of action this token authorizes.
    pub action_type: ActionTokenType,
    /// Additional context data for the token action (JSON, 2B-4KB).
    pub action_data: serde_json::Value,
    /// IP address where the token was generated.
    pub ip_address: IpNet,
    /// User agent of the client that generated the token.
    pub user_agent: String,
    /// Optional device identifier for additional security tracking.
    pub device_id: Option<String>,
    /// Number of times this token has been attempted.
    pub attempt_count: i32,
    /// Maximum allowed attempts before token becomes invalid.
    pub max_attempts: i32,
    /// Timestamp when the token was created.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires.
    pub expired_at: Timestamp,
    /// Timestamp when the token was successfully used.
    pub used_at: Option<Timestamp>,
}

/// Data for creating a new account action token.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = account_action_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccountActionToken {
    /// Reference to the account this token belongs to.
    pub account_id: Uuid,
    /// Type of action this token authorizes.
    pub action_type: ActionTokenType,
    /// Additional context data for the token action.
    pub action_data: Option<serde_json::Value>,
    /// IP address where the token was generated.
    pub ip_address: IpNet,
    /// User agent of the client that generated the token.
    pub user_agent: String,
    /// Optional device identifier for additional security tracking.
    pub device_id: Option<String>,
    /// Maximum allowed attempts before token becomes invalid.
    pub max_attempts: Option<i32>,
    /// Timestamp when the token expires.
    pub expired_at: Option<Timestamp>,
}

/// Data for updating an account action token.
#[derive(Debug, Default, Clone, AsChangeset)]
#[diesel(table_name = account_action_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccountActionToken {
    /// Number of times this token has been attempted.
    pub attempt_count: Option<i32>,
    /// Timestamp when the token was successfully used.
    pub used_at: Option<Option<Timestamp>>,
}

impl AccountActionToken {
    /// Returns whether the token is currently valid (not expired and not used).
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_used() && !self.has_exceeded_attempts()
    }

    /// Returns whether the token has expired.
    pub fn is_expired(&self) -> bool {
        jiff::Timestamp::now() > jiff::Timestamp::from(self.expired_at)
    }

    /// Returns whether the token has been successfully used.
    pub fn is_used(&self) -> bool {
        self.used_at.is_some()
    }

    /// Returns whether the token has exceeded maximum attempts.
    pub fn has_exceeded_attempts(&self) -> bool {
        self.attempt_count >= self.max_attempts
    }

    /// Returns whether the token can be used for another attempt.
    pub fn can_attempt(&self) -> bool {
        self.is_valid() && !self.has_exceeded_attempts()
    }

    /// Returns whether the token has been attempted at least once.
    pub fn has_been_attempted(&self) -> bool {
        self.attempt_count > 0
    }

    /// Returns whether the token is for password reset.
    pub fn is_password_reset(&self) -> bool {
        matches!(self.action_type, ActionTokenType::ResetPassword)
    }

    /// Returns whether the token is for email verification.
    pub fn is_email_verification(&self) -> bool {
        matches!(self.action_type, ActionTokenType::ActivateAccount)
    }

    /// Returns whether the token is for login verification.
    pub fn is_login_verification(&self) -> bool {
        matches!(self.action_type, ActionTokenType::LoginVerification)
    }

    /// Returns the number of remaining attempts.
    pub fn remaining_attempts(&self) -> i32 {
        (self.max_attempts - self.attempt_count).max(0)
    }

    /// Returns a shortened version of the token for logging purposes.
    pub fn action_token_short(&self) -> String {
        let token_str = self.action_token.to_string();
        if token_str.len() > 8 {
            format!("{}...", &token_str[..8])
        } else {
            token_str
        }
    }

    /// Returns whether the token has a device identifier.
    pub fn has_device_id(&self) -> bool {
        self.device_id.is_some()
    }

    /// Returns the token's usage rate (attempts/max_attempts).
    pub fn usage_rate(&self) -> f64 {
        if self.max_attempts > 0 {
            self.attempt_count as f64 / self.max_attempts as f64
        } else {
            0.0
        }
    }

    /// Returns whether the token is close to attempt limit.
    pub fn is_near_attempt_limit(&self) -> bool {
        self.usage_rate() >= 0.8 // 80% of attempts used
    }

    /// Returns whether the token requires immediate action.
    pub fn requires_immediate_action(&self) -> bool {
        matches!(
            self.action_type,
            ActionTokenType::ResetPassword | ActionTokenType::LoginVerification
        )
    }

    /// Returns the recommended expiry time for this token type.
    pub fn recommended_expiry_duration(&self) -> jiff::Span {
        match self.action_type {
            ActionTokenType::ActivateAccount => jiff::Span::new().hours(24),
            ActionTokenType::ResetPassword => jiff::Span::new().hours(1),
            ActionTokenType::LoginVerification => jiff::Span::new().minutes(15),
            _ => jiff::Span::new().hours(2),
        }
    }

    /// Returns whether the token has context data.
    pub fn has_context_data(&self) -> bool {
        !self
            .action_data
            .as_object()
            .is_none_or(|obj| obj.is_empty())
    }

    /// Returns whether the token is stale (created more than recommended duration ago but not expired).
    pub fn is_stale(&self) -> bool {
        !self.is_expired()
            && self.creation_age().total(jiff::Unit::Second).ok()
                > self
                    .recommended_expiry_duration()
                    .total(jiff::Unit::Second)
                    .ok()
    }

    /// Returns whether the token is from a suspicious source.
    pub fn is_suspicious(&self) -> bool {
        // Simple heuristics for suspicious tokens
        self.attempt_count > (self.max_attempts / 2) && !self.is_used()
    }

    /// Returns whether the token has the specified action type.
    pub fn has_action_type(&self, action_type: ActionTokenType) -> bool {
        self.action_type == action_type
    }

    /// Returns whether the token can be refreshed/extended.
    pub fn can_be_refreshed(&self) -> bool {
        self.is_valid() && !self.is_suspicious()
    }
}

impl HasCreatedAt for AccountActionToken {
    fn created_at(&self) -> jiff::Timestamp {
        self.issued_at.into()
    }
}

impl HasExpiresAt for AccountActionToken {
    fn expires_at(&self) -> Option<jiff::Timestamp> {
        Some(self.expired_at.into())
    }
}

impl HasSecurityContext for AccountActionToken {
    fn ip_address(&self) -> Option<IpNet> {
        Some(self.ip_address)
    }

    fn user_agent(&self) -> Option<&str> {
        Some(&self.user_agent)
    }
}
