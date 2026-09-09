//! Account identity (credential) request types.

use garde::Validate;
use nvisy_postgres::types::IdentityProvider;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Path parameters for a provider-scoped identity operation: signing in with,
/// re-authenticating with, linking, or unlinking a provider. Named identically to
/// the stored [`IdentityProvider`], so the API and the account's identities name
/// each provider the same way.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentityPathParams {
    /// The identity provider to act on.
    pub provider: IdentityProvider,
}

/// A password set or change.
///
/// When the account already has a password, `current_password` is required and
/// verified before the change is applied, so a hijacked session or CSRF cannot
/// silently reset it (and lock out the real owner). When the account has no
/// password yet (an SSO-only account setting its first one), there is nothing to
/// re-authenticate against, so a fresh step-up `reauth_proof` is required instead
/// — a live session alone must not mint a durable new credential.
#[must_use]
#[derive(Debug, Serialize, Deserialize, Validate, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct SetPassword {
    /// The account's current password. Required when the account already has a
    /// password; omitted when setting a first password on an account that has
    /// none (supply `reauth_proof` instead).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_password: Option<String>,
    /// A step-up re-authentication proof (from the OIDC reauth endpoint).
    /// Required when *setting a first password* on an account that has none;
    /// ignored when changing an existing password.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reauth_proof: Option<String>,
    /// The new password (will be hashed before storage).
    #[garde(length(chars, min = 8, max = 128))]
    pub new_password: String,
}
