//! Main account model for PostgreSQL database operations.
//!
//! This module provides the core account model for user authentication and management.
//! It handles all aspects of user accounts including authentication, profile management,
//! security features, and account lifecycle operations.
//!
//! ## Models
//!
//! - [`Account`] - Main account model with complete user information and security features
//! - [`NewAccount`] - Data structure for creating new user accounts
//! - [`UpdateAccount`] - Data structure for updating existing account information

use diesel::prelude::*;
use ipnet::IpNet;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::accounts;
use crate::types::constants::account;
use crate::types::{HasCreatedAt, HasDeletedAt, HasSecurityContext, HasUpdatedAt};

/// Main account model representing a user account in the system.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Account {
    /// Unique account identifier.
    pub id: Uuid,
    /// Administrative privileges across the entire system.
    pub is_admin: bool,
    /// Account identity verification status (email confirmation, etc.).
    pub is_verified: bool,
    /// Temporarily disables account access while preserving data.
    pub is_suspended: bool,
    /// Human-readable name for UI and communications (2-100 characters).
    pub display_name: String,
    /// Primary email for authentication and communications (validated format).
    pub email_address: String,
    /// Securely hashed password (bcrypt recommended, minimum 60 characters).
    pub password_hash: String,
    /// Optional company affiliation for business accounts.
    pub company_name: Option<String>,
    /// Optional phone number for 2FA or emergency contact.
    pub phone_number: Option<String>,
    /// Optional URL to profile avatar image.
    pub avatar_url: Option<String>,
    /// Timezone identifier (e.g., "America/New_York", "UTC").
    pub timezone: String,
    /// Preferred locale code (ISO 639-1, e.g., "en", "es", "fr").
    pub locale: String,
    /// Number of consecutive failed login attempts.
    pub failed_login_attempts: i32,
    /// Timestamp until which the account is locked due to failed attempts.
    pub locked_until: Option<Timestamp>,
    /// Timestamp when password was last changed.
    pub password_changed_at: Option<Timestamp>,
    /// Timestamp when the account was created.
    pub created_at: Timestamp,
    /// Timestamp when the account was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the account was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

/// Data for creating a new account.
#[derive(Debug, Default, Clone, Insertable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccount {
    /// Human-readable name for UI and communications (2-100 characters).
    pub display_name: String,
    /// Primary email for authentication and communications (validated format).
    pub email_address: String,
    /// Securely hashed password (bcrypt recommended, minimum 60 characters).
    pub password_hash: String,
    /// Optional company affiliation for business accounts.
    pub company_name: Option<String>,
    /// Optional phone number for 2FA or emergency contact.
    pub phone_number: Option<String>,
    /// Optional URL to profile avatar image.
    pub avatar_url: Option<String>,
    /// Timezone identifier.
    pub timezone: Option<String>,
    /// Preferred locale code.
    pub locale: Option<String>,
}

/// Data for updating an account.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateAccount {
    /// Human-readable name for UI and communications.
    pub display_name: Option<String>,
    /// Primary email for authentication and communications.
    pub email_address: Option<String>,
    /// Securely hashed password.
    pub password_hash: Option<String>,
    /// Company affiliation for business accounts.
    pub company_name: Option<String>,
    /// Phone number for 2FA or emergency contact.
    pub phone_number: Option<String>,
    /// URL to profile avatar image.
    pub avatar_url: Option<String>,
    /// Timezone identifier.
    pub timezone: Option<String>,
    /// Preferred locale code.
    pub locale: Option<String>,
    /// Administrative privileges.
    pub is_admin: Option<bool>,
    /// Account identity verification status.
    pub is_verified: Option<bool>,
    /// Account suspension status.
    pub is_suspended: Option<bool>,
    /// Number of consecutive failed login attempts.
    pub failed_login_attempts: Option<i32>,
    /// Timestamp until which the account is locked.
    pub locked_until: Option<Timestamp>,
    /// Timestamp when password was last changed.
    pub password_changed_at: Option<Timestamp>,
}

impl Account {
    /// Returns whether the account is active and can be used.
    pub fn is_active(&self) -> bool {
        !self.is_suspended && !self.is_deleted() && !self.is_locked()
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

    /// Returns whether the account is currently locked due to failed login attempts.
    pub fn is_locked(&self) -> bool {
        if let Some(locked_until) = self.locked_until {
            jiff::Timestamp::from(locked_until) > jiff::Timestamp::now()
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

    /// Returns whether the account has an avatar URL configured.
    pub fn has_avatar(&self) -> bool {
        self.avatar_url.is_some()
    }

    /// Returns whether the account requires email verification.
    pub fn needs_verification(&self) -> bool {
        !self.is_verified
    }

    /// Returns whether the account is eligible for suspension.
    ///
    /// Only active, non-admin accounts can be suspended. Admin accounts have
    /// protection against suspension to prevent system lockout scenarios.
    pub fn can_be_suspended(&self) -> bool {
        self.is_active() && !self.is_admin()
    }

    /// Returns whether the account is eligible for reactivation from suspension.
    ///
    /// Only suspended accounts that haven't been deleted can be unsuspended.
    pub fn can_be_unsuspended(&self) -> bool {
        self.is_suspended() && !self.is_deleted()
    }

    /// Returns whether the account has too many failed login attempts.
    pub fn has_too_many_failed_attempts(&self) -> bool {
        self.failed_login_attempts >= account::MAX_FAILED_LOGIN_ATTEMPTS
    }

    /// Returns the time remaining until the account lockout expires.
    ///
    /// When an account is temporarily locked due to failed login attempts,
    /// this method calculates how much time remains before automatic unlock.
    pub fn time_until_unlock(&self) -> Option<jiff::Span> {
        if let Some(locked_until) = self.locked_until {
            let now = jiff::Timestamp::now();
            let locked_until = jiff::Timestamp::from(locked_until);
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

impl HasCreatedAt for Account {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for Account {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

impl HasDeletedAt for Account {
    fn deleted_at(&self) -> Option<jiff::Timestamp> {
        self.deleted_at.map(Into::into)
    }
}

impl HasSecurityContext for Account {
    fn ip_address(&self) -> Option<IpNet> {
        None
    }

    fn user_agent(&self) -> Option<&str> {
        None
    }
}
