//! Account response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use nvisy_postgres::types::Username;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents an account.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Public handle of the account.
    pub username: Username,
    /// Whether the account email has been verified.
    pub is_activated: bool,
    /// Whether the account has administrator privileges.
    pub is_admin: bool,
    /// Whether the account is currently suspended.
    pub is_suspended: bool,

    /// Display name of the account holder, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Email address associated with the account.
    pub email_address: String,

    /// Timestamp when the account was created.
    pub created_at: Timestamp,
    /// Timestamp when the account was last updated.
    pub updated_at: Timestamp,
}

impl Account {
    pub fn from_model(account: model::Account) -> Self {
        Self {
            username: account.username,
            is_activated: account.is_verified,
            is_admin: account.is_admin,
            is_suspended: account.is_suspended,

            display_name: account.display_name,
            email_address: account.email_address,

            created_at: account.created_at.into(),
            updated_at: account.updated_at.into(),
        }
    }
}

/// Public view of an account, returned when looking up someone other than the
/// authenticated caller. Carries only the fields safe to share with a
/// workspace peer; private details (email, account flags) are omitted
/// and remain available solely through the caller's own `/account/` view.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    /// Public handle of the account.
    pub username: Username,
    /// Display name of the account holder, when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Timestamp when the account was created.
    pub created_at: Timestamp,
}

impl PublicAccount {
    pub fn from_model(account: model::Account) -> Self {
        Self {
            username: account.username,
            display_name: account.display_name,
            created_at: account.created_at.into(),
        }
    }
}
