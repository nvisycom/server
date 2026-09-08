//! Account identities table constraint violations.

use strum::EnumString;

/// Account identities table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum AccountIdentityConstraints {
    #[strum(serialize = "account_identities_password_shape")]
    PasswordShape,
    #[strum(serialize = "account_identities_oidc_shape")]
    OidcShape,
    #[strum(serialize = "account_identities_account_provider_unique_idx")]
    AccountProviderUnique,
    #[strum(serialize = "account_identities_provider_subject_unique_idx")]
    ProviderSubjectUnique,
}
