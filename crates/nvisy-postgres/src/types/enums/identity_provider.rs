//! Identity provider enumeration for account authentication methods.

use super::db_enum;

db_enum! {
    /// How an account authenticates.
    ///
    /// Corresponds to the `IDENTITY_PROVIDER` PostgreSQL enum. [`Password`] is a
    /// locally-held Argon2 secret; the rest are external OIDC providers keyed by
    /// the provider's subject claim.
    ///
    /// [`Password`]: Self::Password
    pub enum IdentityProvider: Default = Password, "crate::schema::sql_types::IdentityProvider" {
        /// Local password, stored as an Argon2 hash.
        Password = "password",
        /// Google (OIDC).
        Google = "google",
        /// Microsoft / Entra ID (OIDC).
        Microsoft = "microsoft",
    }
}

impl IdentityProvider {
    /// Whether this provider is an external OIDC provider (as opposed to a local
    /// password).
    #[must_use]
    pub fn is_oidc(self) -> bool {
        !matches!(self, Self::Password)
    }
}
