//! Account request types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Request payload to update an account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "displayName": "Jane Smith",
    "emailAddress": "jane.smith@example.com",
    "password": "NewSecurePassword456!",
    "companyName": "Acme Corporation",
    "phoneNumber": "+1-555-0123"
}))]
pub struct UpdateAccount {
    /// New display name (2-32 characters).
    #[validate(length(min = 2, max = 32))]
    pub display_name: Option<String>,

    /// New email address (must be valid email format).
    #[validate(email)]
    pub email_address: Option<String>,

    /// New password (will be hashed before storage).
    pub password: Option<String>,

    /// Company or organization name.
    #[validate(length(max = 100))]
    pub company_name: Option<String>,

    /// Phone number in international format.
    #[validate(length(max = 20))]
    pub phone_number: Option<String>,
}
