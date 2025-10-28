//! Account response types.

use nvisy_postgres::model::Account;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

/// Response returned when retrieving an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetAccountResponse {
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

impl GetAccountResponse {
    /// Creates a new instance of [`GetAccountResponse`].
    pub fn new(account: Account) -> Self {
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

impl From<Account> for GetAccountResponse {
    fn from(account: Account) -> Self {
        Self::new(account)
    }
}

/// Response returned after updating an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountResponse {
    /// Unique identifier of the updated account.
    pub account_id: Uuid,

    /// Timestamp when the account was created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the account was last updated.
    pub updated_at: OffsetDateTime,
}

impl UpdateAccountResponse {
    /// Creates a new instance of [`UpdateAccountResponse`].
    pub fn new(account: Account) -> Self {
        Self {
            account_id: account.id,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

impl From<Account> for UpdateAccountResponse {
    fn from(account: Account) -> Self {
        Self::new(account)
    }
}

/// Response returned after deleting an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountResponse {
    /// Unique identifier of the deleted account.
    pub account_id: Uuid,

    /// Timestamp when the account was originally created.
    pub created_at: OffsetDateTime,
    /// Timestamp when the account was deleted.
    pub deleted_at: Option<OffsetDateTime>,
}

impl DeleteAccountResponse {
    /// Creates a new instance of [`DeleteAccountResponse`].
    pub fn new(account: Account) -> Self {
        Self {
            account_id: account.id,
            created_at: account.created_at,
            deleted_at: account.deleted_at,
        }
    }
}

impl From<Account> for DeleteAccountResponse {
    fn from(account: Account) -> Self {
        Self::new(account)
    }
}
