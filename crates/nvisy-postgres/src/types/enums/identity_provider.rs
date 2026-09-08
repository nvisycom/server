//! Identity provider enumeration for account authentication methods.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// How an account authenticates.
///
/// Corresponds to the `IDENTITY_PROVIDER` PostgreSQL enum. [`Password`] is a
/// locally-held Argon2 secret; the rest are external OIDC providers keyed by the
/// provider's subject claim.
///
/// [`Password`]: Self::Password
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::IdentityProvider"]
pub enum IdentityProvider {
    /// Local password, stored as an Argon2 hash.
    #[db_rename = "password"]
    #[serde(rename = "password")]
    #[default]
    Password,

    /// Google (OIDC).
    #[db_rename = "google"]
    #[serde(rename = "google")]
    Google,

    /// Microsoft / Entra ID (OIDC).
    #[db_rename = "microsoft"]
    #[serde(rename = "microsoft")]
    Microsoft,
}

impl IdentityProvider {
    /// Whether this provider is an external OIDC provider (as opposed to a local
    /// password).
    #[must_use]
    pub fn is_oidc(self) -> bool {
        !matches!(self, Self::Password)
    }
}
