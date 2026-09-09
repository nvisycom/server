//! Account request types.

use nvisy_postgres::model::UpdateAccount as UpdateAccountModel;
use nvisy_postgres::types::Handle;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// Request payload to update an account's profile.
///
/// Credentials are not profile fields — a password and any linked providers are
/// managed through the account's identities (see the identity endpoints), never
/// here.
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
}

impl UpdateAccount {
    /// Converts this request into a database model for the profile fields.
    pub fn into_model(self) -> UpdateAccountModel {
        UpdateAccountModel {
            username: self.username,
            display_name: self.display_name.map(Some),
            email_address: self.email_address,
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
