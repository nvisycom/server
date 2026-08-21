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
use crate::types::{Handle, HasCreatedAt, HasDeletedAt, HasSecurityContext, HasUpdatedAt};

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
    /// Public account handle, unique across all accounts.
    pub username: Handle,
    /// Optional human-readable name for UI and communications (2-100 chars).
    pub display_name: Option<String>,
    /// Primary email for authentication and communications (validated format).
    pub email_address: String,
    /// Securely hashed password (bcrypt recommended, minimum 60 characters).
    pub password_hash: String,
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

/// Data for creating a new account.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccount {
    /// Public account handle, unique across all accounts.
    pub username: Handle,
    /// Optional human-readable name for UI and communications (2-100 chars).
    pub display_name: Option<String>,
    /// Primary email for authentication and communications (validated format).
    pub email_address: String,
    /// Securely hashed password (bcrypt recommended, minimum 60 characters).
    pub password_hash: String,
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
    /// Public account handle, unique across all accounts.
    pub username: Option<Handle>,
    /// Human-readable name for UI and communications (`Some(None)` clears it).
    pub display_name: Option<Option<String>>,
    /// Primary email for authentication and communications.
    pub email_address: Option<String>,
    /// Securely hashed password.
    pub password_hash: Option<String>,
    /// URL to profile avatar image (`Some(None)` clears it).
    pub avatar_url: Option<Option<String>>,
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
    /// Timestamp when password was last changed.
    pub password_changed_at: Option<Timestamp>,
}

impl Account {
    /// Returns whether the account is suspended.
    pub fn is_suspended(&self) -> bool {
        self.is_suspended
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
