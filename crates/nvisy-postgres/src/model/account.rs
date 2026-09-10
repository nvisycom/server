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
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::accounts;
use crate::types::Handle;

/// Main account model representing a user account in the system.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Account {
    /// Unique account identifier.
    pub id: Uuid,
    /// Account identity verification status (email confirmation, etc.).
    pub is_verified: bool,
    /// Temporarily disables account access while preserving data.
    pub is_suspended: bool,
    /// Public account handle, unique across all accounts.
    pub username: Handle,
    /// Optional human-readable name for UI and communications (2-100 chars).
    pub display_name: Option<String>,
    /// Primary email for authentication and communications (validated format).
    pub email_address: String,
    /// Optional URL to profile avatar image.
    pub avatar_url: Option<String>,
    /// Timezone identifier (e.g., "America/New_York", "UTC").
    pub timezone: String,
    /// Preferred locale code (ISO 639-1, e.g., "en", "es", "fr").
    pub locale: String,
    /// Timestamp when password was last changed.
    pub password_changed_at: Option<Timestamp>,
    /// Timestamp when the account was created.
    pub created_at: Timestamp,
    /// Timestamp when the account was last updated.
    pub updated_at: Timestamp,
    /// Timestamp when the account was soft-deleted.
    pub deleted_at: Option<Timestamp>,
}

impl Account {
    /// An account row with a fresh unique handle and matching email, for tests.
    /// Not verified/suspended; timestamps are now. Useful to downstream crates
    /// that need an `Account` without a database.
    #[cfg(any(feature = "test_util", test))]
    #[must_use]
    pub fn test() -> Self {
        let username = Handle::test();
        let email_address = format!("{}@example.com", username.as_str());
        let now: Timestamp = jiff::Timestamp::now().into();
        Self {
            id: Uuid::now_v7(),
            is_verified: false,
            is_suspended: false,
            username,
            display_name: None,
            email_address,
            avatar_url: None,
            timezone: "UTC".to_owned(),
            locale: "en".to_owned(),
            password_changed_at: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }
}

/// Data for creating a new account.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct NewAccount {
    /// Public account handle, unique across all accounts.
    pub username: Handle,
    /// Optional human-readable name for UI and communications (2-100 chars).
    pub display_name: Option<String>,
    /// Primary email for authentication and communications (validated format).
    pub email_address: String,
    /// Optional URL to profile avatar image.
    pub avatar_url: Option<String>,
    /// Timezone identifier.
    pub timezone: Option<String>,
    /// Preferred locale code.
    pub locale: Option<String>,
}

impl NewAccount {
    /// Creates an account with the required handle and email; all optional
    /// profile fields default to `None`.
    pub fn new(username: Handle, email_address: impl Into<String>) -> Self {
        Self {
            username,
            display_name: None,
            email_address: email_address.into(),
            avatar_url: None,
            timezone: None,
            locale: None,
        }
    }

    /// An account with a fresh unique handle and matching email, for tests.
    #[cfg(any(feature = "test_util", test))]
    pub fn test() -> Self {
        let handle = Handle::test();
        let email = format!("{}@example.com", handle.as_str());
        Self::new(handle, email)
    }
}

/// Data for updating an account.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[must_use]
pub struct UpdateAccount {
    /// Public account handle, unique across all accounts.
    pub username: Option<Handle>,
    /// Human-readable name for UI and communications (`Some(None)` clears it).
    pub display_name: Option<Option<String>>,
    /// Primary email for authentication and communications.
    pub email_address: Option<String>,
    /// URL to profile avatar image (`Some(None)` clears it).
    pub avatar_url: Option<Option<String>>,
    /// Timezone identifier.
    pub timezone: Option<String>,
    /// Preferred locale code.
    pub locale: Option<String>,
    /// Account identity verification status.
    pub is_verified: Option<bool>,
    /// Account suspension status.
    pub is_suspended: Option<bool>,
    /// Timestamp when password was last changed.
    pub password_changed_at: Option<Timestamp>,
}

impl Account {
    /// Returns whether the account is suspended.
    pub fn is_suspended(&self) -> bool {
        self.is_suspended
    }

    /// Returns whether the account has been soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}
