//! Account response types.

use jiff::Timestamp;
use nvisy_postgres::model;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents an account.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Unique identifier of the account.
    pub account_id: Uuid,
    /// Whether the account email has been verified.
    pub is_activated: bool,
    /// Whether the account has administrator privileges.
    pub is_admin: bool,
    /// Whether the account is currently suspended.
    pub is_suspended: bool,

    /// Display name of the account holder.
    pub display_name: String,
    /// Email address associated with the account.
    pub email_address: String,
    /// Company name (optional).
    pub company_name: Option<String>,

    /// Timestamp when the account was created.
    pub created_at: Timestamp,
    /// Timestamp when the account was last updated.
    pub updated_at: Timestamp,
}

impl Account {
    pub fn from_model(account: model::Account) -> Self {
        Self {
            account_id: account.id,
            is_activated: account.is_verified,
            is_admin: account.is_admin,
            is_suspended: account.is_suspended,

            display_name: account.display_name,
            email_address: account.email_address,
            company_name: account.company_name,

            created_at: account.created_at.into(),
            updated_at: account.updated_at.into(),
        }
    }
}
