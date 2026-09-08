//! Account identity (credential) response types.

use jiff::Timestamp;
use nvisy_postgres::model::AccountIdentity as AccountIdentityModel;
use nvisy_postgres::types::IdentityProvider;
use schemars::JsonSchema;
use serde::Serialize;

/// One of an account's sign-in methods, for the identities listing.
///
/// Never exposes the credential itself (a password hash or a provider subject),
/// only which methods exist and metadata about them.
#[must_use]
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountIdentity {
    /// The authentication method: `password` or an OIDC provider.
    pub provider: IdentityProvider,
    /// The email the provider asserted at link time, for a linked provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_email: Option<String>,
    /// When the identity was created (the password set, or the provider linked).
    pub created_at: Timestamp,
    /// When the identity was last updated.
    pub updated_at: Timestamp,
}

impl AccountIdentity {
    /// Builds the response view from a stored identity, dropping the secret and
    /// provider subject (never exposed).
    pub fn from_model(model: AccountIdentityModel) -> Self {
        Self {
            provider: model.provider,
            provider_email: model.provider_email,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }
}

/// The account's sign-in methods.
#[must_use]
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountIdentities {
    /// Every identity the account can sign in with.
    pub identities: Vec<AccountIdentity>,
}

impl FromIterator<AccountIdentityModel> for AccountIdentities {
    fn from_iter<T: IntoIterator<Item = AccountIdentityModel>>(iter: T) -> Self {
        Self {
            identities: iter.into_iter().map(AccountIdentity::from_model).collect(),
        }
    }
}
