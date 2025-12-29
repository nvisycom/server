//! Account request types.

use nvisy_postgres::model::UpdateAccount as UpdateAccountModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Request payload to update an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccount {
    /// New display name (2-100 characters).
    #[validate(length(min = 2, max = 100))]
    #[validate(custom(function = "validate_display_name_format"))]
    pub display_name: Option<String>,

    /// New email address (must be valid email format).
    #[validate(email)]
    #[validate(length(min = 5, max = 254))]
    pub email_address: Option<String>,

    /// New password (will be hashed before storage).
    #[validate(length(min = 8, max = 256))]
    pub password: Option<String>,

    /// Company or organization name.
    #[validate(length(min = 2, max = 100))]
    pub company_name: Option<String>,

    /// Phone number.
    #[validate(length(min = 10, max = 20))]
    #[validate(custom(function = "validate_phone_format"))]
    pub phone_number: Option<String>,
}

impl UpdateAccount {
    /// Converts this request into a database model.
    ///
    /// Note: Password must be hashed separately before setting `password_hash`.
    pub fn into_model(self, password_hash: Option<String>) -> UpdateAccountModel {
        UpdateAccountModel {
            display_name: self.display_name,
            email_address: self.email_address,
            password_hash,
            company_name: self.company_name,
            phone_number: self.phone_number,
            ..Default::default()
        }
    }
}

fn validate_phone_format(phone: &str) -> Result<(), ValidationError> {
    let cleaned: String = phone
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();

    if cleaned.chars().filter(|c| c.is_ascii_digit()).count() < 7 {
        return Err(ValidationError::new("phone_format"));
    }
    Ok(())
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
