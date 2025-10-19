//! Action token type enumeration for secure operations and verifications.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of action that a security token authorizes.
///
/// This enumeration corresponds to the `ACTION_TOKEN_TYPE` PostgreSQL enum and is used
/// for various token-based security operations including account verification,
/// password management, and data operations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::ActionTokenType"]
pub enum ActionTokenType {
    /// Email verification for new account activation
    #[db_rename = "activate_account"]
    #[serde(rename = "activate_account")]
    ActivateAccount,

    /// Account suspension or deactivation authorization
    #[db_rename = "deactivate_account"]
    #[serde(rename = "deactivate_account")]
    DeactivateAccount,

    /// Email address change verification
    #[db_rename = "update_email"]
    #[serde(rename = "update_email")]
    UpdateEmail,

    /// Password reset via email link
    #[db_rename = "reset_password"]
    #[serde(rename = "reset_password")]
    ResetPassword,

    /// Password change verification (for existing users)
    #[db_rename = "change_password"]
    #[serde(rename = "change_password")]
    ChangePassword,

    /// Two-factor authentication setup
    #[db_rename = "enable_2fa"]
    #[serde(rename = "enable_2fa")]
    Enable2fa,

    /// Two-factor authentication removal
    #[db_rename = "disable_2fa"]
    #[serde(rename = "disable_2fa")]
    Disable2fa,

    /// Additional login verification (suspicious activity)
    #[db_rename = "login_verification"]
    #[serde(rename = "login_verification")]
    LoginVerification,

    /// API access token generation
    #[db_rename = "api_access"]
    #[serde(rename = "api_access")]
    ApiAccess,

    /// Data import authorization
    #[db_rename = "import_data"]
    #[serde(rename = "import_data")]
    ImportData,

    /// Data export authorization
    #[db_rename = "export_data"]
    #[serde(rename = "export_data")]
    ExportData,
}

impl ActionTokenType {
    /// Returns whether this token action requires user authentication.
    #[inline]
    pub fn requires_authentication(self) -> bool {
        matches!(
            self,
            ActionTokenType::DeactivateAccount
                | ActionTokenType::UpdateEmail
                | ActionTokenType::ChangePassword
                | ActionTokenType::Enable2fa
                | ActionTokenType::Disable2fa
                | ActionTokenType::ApiAccess
                | ActionTokenType::ImportData
                | ActionTokenType::ExportData
        )
    }

    /// Returns whether this token action is related to account security.
    #[inline]
    pub fn is_security_related(self) -> bool {
        matches!(
            self,
            ActionTokenType::ActivateAccount
                | ActionTokenType::DeactivateAccount
                | ActionTokenType::ResetPassword
                | ActionTokenType::ChangePassword
                | ActionTokenType::Enable2fa
                | ActionTokenType::Disable2fa
                | ActionTokenType::LoginVerification
        )
    }

    /// Returns whether this token action involves data operations.
    #[inline]
    pub fn is_data_operation(self) -> bool {
        matches!(
            self,
            ActionTokenType::ImportData | ActionTokenType::ExportData
        )
    }

    /// Returns the default expiration time in seconds for this token action.
    #[inline]
    pub fn default_expiration_seconds(self) -> u32 {
        match self {
            // Short-lived tokens for sensitive operations
            ActionTokenType::LoginVerification => 300, // 5 minutes
            ActionTokenType::Enable2fa | ActionTokenType::Disable2fa => 300, // 5 minutes

            // Medium-lived tokens for account operations
            ActionTokenType::ActivateAccount => 3600, // 1 hour
            ActionTokenType::UpdateEmail => 3600,     // 1 hour
            ActionTokenType::ResetPassword => 3600,   // 1 hour
            ActionTokenType::ChangePassword => 1800,  // 30 minutes

            // Longer-lived tokens for data operations
            ActionTokenType::ImportData => 7200, // 2 hours
            ActionTokenType::ExportData => 7200, // 2 hours

            // Administrative tokens
            ActionTokenType::DeactivateAccount => 1800, // 30 minutes
            ActionTokenType::ApiAccess => 86400,        // 24 hours
        }
    }
}
