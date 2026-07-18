//! Account request types.

use nvisy_postgres::model::UpdateAccount as UpdateAccountModel;
use nvisy_postgres::types::Username;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Request payload to update an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccount {
    /// New account handle.
    pub username: Option<Username>,
    /// New display name (2-32 characters; empty string clears it).
    #[validate(length(max = 32))]
    #[validate(custom(function = "validate_display_name_format"))]
    pub display_name: Option<String>,
    /// New email address (must be valid email format).
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: Option<String>,
    /// New password (will be hashed before storage).
    #[validate(length(min = 8, max = 256))]
    pub password: Option<String>,
    /// Company or organization name (empty string clears the value).
    #[validate(length(max = 100))]
    pub company_name: Option<String>,
}

impl UpdateAccount {
    /// Converts this request into a database model.
    ///
    /// Note: Password must be hashed separately before setting `password_hash`.
    /// A supplied empty `display_name` clears the stored value.
    pub fn into_model(self, password_hash: Option<String>) -> UpdateAccountModel {
        UpdateAccountModel {
            username: self.username,
            display_name: self
                .display_name
                .map(|name| (!name.trim().is_empty()).then_some(name)),
            email_address: self.email_address,
            password_hash,
            company_name: self.company_name.map(Some),
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
