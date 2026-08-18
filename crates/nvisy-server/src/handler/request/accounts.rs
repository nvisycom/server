//! Account request types.

use nvisy_postgres::model::UpdateAccount as UpdateAccountModel;
use nvisy_postgres::types::Handle;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// A re-authenticated password change: the new password plus the current one
/// that authorizes it.
///
/// Coupling the two in one struct makes the invariant explicit — a password
/// change always carries the current password, so a hijacked session or CSRF
/// cannot silently reset it (and lock out the real owner).
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChange {
    /// The account's current password, verified before the change is applied.
    pub current_password: String,
    /// The new password (will be hashed before storage).
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

/// Request payload to update an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccount {
    /// New account handle.
    pub username: Option<Handle>,
    /// New display name (2-32 characters).
    #[validate(length(min = 2, max = 32))]
    #[validate(custom(function = "validate_display_name_format"))]
    pub display_name: Option<String>,
    /// New email address (must be valid email format).
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: Option<String>,
    /// A re-authenticated password change, when changing the password.
    #[validate(nested)]
    pub password: Option<PasswordChange>,
}

impl UpdateAccount {
    /// Converts this request into a database model.
    ///
    /// Note: Password must be hashed separately before setting `password_hash`.
    pub fn into_model(self, password_hash: Option<String>) -> UpdateAccountModel {
        UpdateAccountModel {
            username: self.username,
            display_name: self.display_name.map(Some),
            email_address: self.email_address,
            password_hash,
            ..Default::default()
        }
    }
}

fn validate_display_name_format(name: &str) -> Result<(), ValidationError> {
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '\'')
    {
        return Err(ValidationError::new("display_name_format"));
    }
    Ok(())
}
