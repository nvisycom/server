//! Account identity model for PostgreSQL database operations.
//!
//! An account identity is one way an account authenticates: a local password
//! (an Argon2 hash in [`secret`](AccountIdentity::secret)) or a linked OIDC
//! provider (keyed by the provider's [`provider_subject`](AccountIdentity::provider_subject)).
//! Credentials live here rather than on the account, so authentication methods
//! are decoupled from identity.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use uuid::Uuid;

use crate::schema::account_identities;
use crate::types::{HasCreatedAt, HasUpdatedAt, IdentityProvider};

/// One authentication method for an account.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = account_identities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountIdentity {
    /// Unique identity identifier.
    pub id: Uuid,
    /// Account this identity authenticates.
    pub account_id: Uuid,
    /// Authentication method: a local password or an OIDC provider.
    pub provider: IdentityProvider,
    /// Argon2 password hash for a password identity; `None` for OIDC.
    pub secret: Option<String>,
    /// OIDC provider subject (`sub`) claim; `None` for a password identity.
    pub provider_subject: Option<String>,
    /// Email the provider asserted at link time; `None` for a password identity.
    pub provider_email: Option<String>,
    /// Timestamp when the identity was created.
    pub created_at: Timestamp,
    /// Timestamp when the identity was last updated.
    pub updated_at: Timestamp,
}

/// Data for creating a new account identity.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = account_identities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewAccountIdentity {
    /// Account this identity authenticates.
    pub account_id: Uuid,
    /// Authentication method: a local password or an OIDC provider.
    pub provider: IdentityProvider,
    /// Argon2 password hash for a password identity; `None` for OIDC.
    pub secret: Option<String>,
    /// OIDC provider subject (`sub`) claim; `None` for a password identity.
    pub provider_subject: Option<String>,
    /// Email the provider asserted at link time; `None` for a password identity.
    pub provider_email: Option<String>,
}

impl NewAccountIdentity {
    /// Builds a local password identity holding an Argon2 hash.
    #[must_use]
    pub fn password(account_id: Uuid, secret: String) -> Self {
        Self {
            account_id,
            provider: IdentityProvider::Password,
            secret: Some(secret),
            provider_subject: None,
            provider_email: None,
        }
    }

    /// Builds an OIDC identity keyed by the provider's subject claim.
    #[must_use]
    pub fn oidc(
        account_id: Uuid,
        provider: IdentityProvider,
        provider_subject: String,
        provider_email: Option<String>,
    ) -> Self {
        Self {
            account_id,
            provider,
            secret: None,
            provider_subject: Some(provider_subject),
            provider_email,
        }
    }
}

impl HasCreatedAt for AccountIdentity {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for AccountIdentity {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}
