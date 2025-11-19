//! Account request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

/// Request payload to update an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Jane Smith",
    "emailAddress": "jane.smith@example.com",
    "password": "newpassword123",
    "companyName": "Acme Corporation",
    "phoneNumber": "+1-555-0123"
}))]
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
    #[validate(length(min = 8, max = 128))]
    pub password: Option<String>,

    /// Company or organization name.
    #[validate(length(min = 2, max = 100))]
    pub company_name: Option<String>,

    /// Phone number.
    #[validate(length(min = 10, max = 20))]
    #[validate(custom(function = "validate_phone_format"))]
    pub phone_number: Option<String>,
}

fn validate_phone_format(phone: &str) -> Result<(), ValidationError> {
    let cleaned: String = phone
        .chars()
        .filter(|c| c.is_ascii_digit() || "+()- ".contains(*c))
        .collect();
    if cleaned.chars().filter(|c| c.is_ascii_digit()).count() < 7 {
        return Err(ValidationError::new("invalid_phone_format"));
    }
    Ok(())
}

fn validate_display_name_format(name: &str) -> Result<(), ValidationError> {
    if !name.chars().all(|c| c.is_alphanumeric()) {
        return Err(ValidationError::new("display_name_not_alphanumeric"));
    }
    Ok(())
}
