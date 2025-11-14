//! Account response types.

use nvisy_postgres::model;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Represents an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
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
    /// Phone number (optional).
    pub phone_number: Option<String>,

    /// Timestamp when the account was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the account was last updated.
    pub updated_at: OffsetDateTime,
}

impl From<model::Account> for Account {
    fn from(account: model::Account) -> Self {
        Self {
            account_id: account.id,
            is_activated: account.is_verified,
            is_admin: account.is_admin,
            is_suspended: account.is_suspended,

            display_name: account.display_name,
            email_address: account.email_address,
            company_name: account.company_name,
            phone_number: account.phone_number,

            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}
