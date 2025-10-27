//! Main account model for PostgreSQL database operations.

use diesel::prelude::*;
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::schema::accounts;

/// Main account model representing a user account in the system.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Account {
    /// Unique account identifier
    pub id: Uuid,
    /// Administrative privileges across the entire system
    pub is_admin: bool,
    /// Account identity verification status (email confirmation, etc.)
    pub is_verified: bool,
    /// Temporarily disables account access while preserving data
    pub is_suspended: bool,
    /// Human-readable name for UI and communications (2-100 characters)
    pub display_name: String,
    /// Primary email for authentication and communications (validated format)
    pub email_address: String,
    /// Securely hashed password (bcrypt recommended, minimum 60 characters)
    pub password_hash: String,
    /// Optional company affiliation for business accounts
    pub company_name: Option<String>,
    /// Optional phone number for 2FA or emergency contact
    pub phone_number: Option<String>,
    /// Optional URL to profile avatar image
    pub avatar_url: Option<String>,
    /// Timezone identifier (e.g., "America/New_York", "UTC")
    pub timezone: String,
    /// Preferred locale code (ISO 639-1, e.g., "en", "es", "fr")
    pub locale: String,
    /// Number of consecutive failed login attempts
    pub failed_login_attempts: i32,
    /// Timestamp until which the account is locked due to failed attempts
    pub locked_until: Option<OffsetDateTime>,
    /// Timestamp when password was last changed
    pub password_changed_at: Option<OffsetDateTime>,
    /// Timestamp of the last successful login
    pub last_login_at: Option<OffsetDateTime>,
    /// IP address of the last successful login
    pub last_login_ip: Option<IpNet>,
    /// Timestamp when the account was created
    pub created_at: OffsetDateTime,
    /// Timestamp when the account was last updated
    pub updated_at: OffsetDateTime,
    /// Timestamp when the account was soft-deleted
    pub deleted_at: Option<OffsetDateTime>,
}

/// Data for creating a new account.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccount {
    /// Human-readable name for UI and communications (2-100 characters)
    pub display_name: String,
    /// Primary email for authentication and communications (validated format)
    pub email_address: String,
    /// Securely hashed password (bcrypt recommended, minimum 60 characters)
    pub password_hash: String,
    /// Optional company affiliation for business accounts
    pub company_name: String,
    /// Optional phone number for 2FA or emergency contact
    pub phone_number: String,
    /// Optional URL to profile avatar image
    pub avatar_url: Option<String>,
    /// Timezone identifier
    pub timezone: String,
    /// Preferred locale code
    pub locale: String,
}

/// Data for updating an account.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccount {
    /// Human-readable name for UI and communications
    pub display_name: Option<String>,
    /// Primary email for authentication and communications
    pub email_address: Option<String>,
    /// Securely hashed password
    pub password_hash: Option<String>,
    /// Company affiliation for business accounts
    pub company_name: Option<String>,
    /// Phone number for 2FA or emergency contact
    pub phone_number: Option<String>,
    /// URL to profile avatar image
    pub avatar_url: Option<String>,
    /// Timezone identifier
    pub timezone: Option<String>,
    /// Preferred locale code
    pub locale: Option<String>,
    /// Administrative privileges
    pub is_admin: Option<bool>,
    /// Account identity verification status
    pub is_verified: Option<bool>,
    /// Account suspension status
    pub is_suspended: Option<bool>,
    /// Number of consecutive failed login attempts
    pub failed_login_attempts: Option<i32>,
    /// Timestamp until which the account is locked
    pub locked_until: Option<OffsetDateTime>,
    /// Timestamp when password was last changed
    pub password_changed_at: Option<OffsetDateTime>,
    /// Timestamp of the last successful login
    pub last_login_at: Option<OffsetDateTime>,
    /// IP address of the last successful login
    pub last_login_ip: Option<IpNet>,
}

impl Default for NewAccount {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            email_address: String::new(),
            password_hash: String::new(),
            company_name: String::new(),
            phone_number: String::new(),
            avatar_url: None,
            timezone: "UTC".to_string(),
            locale: "en".to_string(),
        }
    }
}

impl Account {
    /// Returns whether the account is active and can be used.
    pub fn is_active(&self) -> bool {
        !self.is_suspended && self.deleted_at.is_none() && !self.is_locked()
    }

    /// Returns whether the account is suspended.
    pub fn is_suspended(&self) -> bool {
        self.is_suspended
    }

    /// Returns whether the account is verified.
    pub fn is_verified(&self) -> bool {
        self.is_verified
    }

    /// Returns whether the account has admin privileges.
    pub fn is_admin(&self) -> bool {
        self.is_admin
    }

    /// Returns whether the account is deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the account is currently locked due to failed login attempts.
    pub fn is_locked(&self) -> bool {
        if let Some(locked_until) = self.locked_until {
            locked_until > time::OffsetDateTime::now_utc()
        } else {
            false
        }
    }

    /// Returns whether the account can log in.
    pub fn can_login(&self) -> bool {
        self.is_active() && self.is_verified() && !self.is_locked()
    }

    /// Returns whether the account can perform admin actions.
    pub fn can_admin(&self) -> bool {
        self.is_active() && self.is_admin()
    }

    /// Returns whether the account has a phone number set.
    pub fn has_phone_number(&self) -> bool {
        self.phone_number
            .as_deref()
            .is_some_and(|phone_number| !phone_number.is_empty())
    }

    /// Returns whether the account has a company name set.
    pub fn has_company(&self) -> bool {
        self.company_name
            .as_deref()
            .is_some_and(|company_name| !company_name.is_empty())
    }

    /// Returns whether the account has an avatar URL set.
    pub fn has_avatar(&self) -> bool {
        self.avatar_url.is_some()
    }

    /// Returns whether the account was created recently (within last 24 hours).
    pub fn is_recently_created(&self) -> bool {
        let now = time::OffsetDateTime::now_utc();
        let duration = now - self.created_at;
        duration.whole_days() < 1
    }

    /// Returns whether the account requires email verification.
    pub fn needs_verification(&self) -> bool {
        !self.is_verified
    }

    /// Returns whether the account can be suspended.
    pub fn can_be_suspended(&self) -> bool {
        self.is_active() && !self.is_admin()
    }

    /// Returns whether the account can be unsuspended.
    pub fn can_be_unsuspended(&self) -> bool {
        self.is_suspended() && !self.is_deleted()
    }

    /// Returns whether the account has too many failed login attempts.
    pub fn has_too_many_failed_attempts(&self) -> bool {
        self.failed_login_attempts >= 5
    }

    /// Returns whether the password was changed recently (within last 90 days).
    pub fn password_recently_changed(&self) -> bool {
        if let Some(changed_at) = self.password_changed_at {
            let now = time::OffsetDateTime::now_utc();
            let duration = now - changed_at;
            duration.whole_days() < 90
        } else {
            false
        }
    }

    /// Returns whether the account was active recently (logged in within last 30 days).
    pub fn recently_active(&self) -> bool {
        if let Some(last_login) = self.last_login_at {
            let now = time::OffsetDateTime::now_utc();
            let duration = now - last_login;
            duration.whole_days() < 30
        } else {
            false
        }
    }

    /// Returns a display name for the account, falling back to email if needed.
    pub fn display_name_or_email(&self) -> &str {
        if self.display_name.is_empty() {
            &self.email_address
        } else {
            &self.display_name
        }
    }

    /// Returns the account's initials for avatar fallbacks.
    pub fn get_initials(&self) -> String {
        let name = if self.display_name.is_empty() {
            &self.email_address
        } else {
            &self.display_name
        };

        name.split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }

    /// Returns the time remaining until the account is unlocked.
    pub fn time_until_unlock(&self) -> Option<time::Duration> {
        if let Some(locked_until) = self.locked_until {
            let now = time::OffsetDateTime::now_utc();
            if locked_until > now {
                Some(locked_until - now)
            } else {
                None
            }
        } else {
            None
        }
    }
}
