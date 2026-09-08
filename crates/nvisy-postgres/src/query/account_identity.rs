//! Account identity repository for managing authentication methods.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{AccountIdentity, NewAccountIdentity};
use crate::types::IdentityProvider;
use crate::{Error, PgConnection, Result, schema};

/// Repository for account identity database operations.
///
/// An account authenticates through one or more identities: a local password
/// (an Argon2 hash) and/or linked OIDC providers. This is the sole home for
/// credentials — the account row holds none.
pub trait AccountIdentityRepository {
    /// Creates a new account identity.
    fn create_account_identity(
        &mut self,
        identity: NewAccountIdentity,
    ) -> impl Future<Output = Result<AccountIdentity>> + Send;

    /// Finds an account's identity for a given provider, if any.
    ///
    /// With the `(account_id, provider)` uniqueness, this returns at most one
    /// row — the account's password identity, or its link to that OIDC provider.
    fn find_account_identity(
        &mut self,
        account_id: Uuid,
        provider: IdentityProvider,
    ) -> impl Future<Output = Result<Option<AccountIdentity>>> + Send;

    /// Finds the identity a returning OIDC user resolves to, by the provider and
    /// its stable subject (`sub`) claim.
    fn find_identity_by_subject(
        &mut self,
        provider: IdentityProvider,
        provider_subject: &str,
    ) -> impl Future<Output = Result<Option<AccountIdentity>>> + Send;

    /// Lists all of an account's identities (its password and linked providers),
    /// ordered by provider for a stable listing.
    fn list_account_identities(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = Result<Vec<AccountIdentity>>> + Send;

    /// Sets an account's password secret, creating the password identity if the
    /// account does not have one yet (an SSO-only account setting a first
    /// password) and replacing it otherwise. Keyed on `(account_id, provider)`,
    /// so it is a single upsert.
    fn upsert_password_secret(
        &mut self,
        account_id: Uuid,
        secret: String,
    ) -> impl Future<Output = Result<AccountIdentity>> + Send;

    /// Deletes an account's identity for `provider`, but only while it is not the
    /// account's *last* identity — an account must always keep at least one way
    /// to authenticate. The guard is applied in the same statement (a delete
    /// conditioned on more than one identity existing) so it is race-free.
    ///
    /// Returns [`DeleteIdentityOutcome`] distinguishing a successful delete, a
    /// refusal to remove the last identity, and a provider that was not linked.
    fn delete_account_identity(
        &mut self,
        account_id: Uuid,
        provider: IdentityProvider,
    ) -> impl Future<Output = Result<DeleteIdentityOutcome>> + Send;
}

/// The result of a [`delete_account_identity`](AccountIdentityRepository::delete_account_identity)
/// call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteIdentityOutcome {
    /// The identity was deleted.
    Deleted,
    /// The identity exists but is the account's only one, so it was kept: an
    /// account must retain at least one way to authenticate.
    LastIdentityKept,
    /// The account has no identity for that provider; nothing to delete.
    NotFound,
}

impl AccountIdentityRepository for PgConnection {
    async fn create_account_identity(
        &mut self,
        identity: NewAccountIdentity,
    ) -> Result<AccountIdentity> {
        use schema::account_identities;

        diesel::insert_into(account_identities::table)
            .values(&identity)
            .returning(AccountIdentity::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn find_account_identity(
        &mut self,
        account_id: Uuid,
        provider: IdentityProvider,
    ) -> Result<Option<AccountIdentity>> {
        use schema::account_identities::dsl;

        dsl::account_identities
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::provider.eq(provider))
            .select(AccountIdentity::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn find_identity_by_subject(
        &mut self,
        provider: IdentityProvider,
        provider_subject: &str,
    ) -> Result<Option<AccountIdentity>> {
        use schema::account_identities::dsl;

        dsl::account_identities
            .filter(dsl::provider.eq(provider))
            .filter(dsl::provider_subject.eq(provider_subject))
            .select(AccountIdentity::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn list_account_identities(&mut self, account_id: Uuid) -> Result<Vec<AccountIdentity>> {
        use schema::account_identities::dsl;

        dsl::account_identities
            .filter(dsl::account_id.eq(account_id))
            .order(dsl::provider.asc())
            .select(AccountIdentity::as_select())
            .load(self)
            .await
            .map_err(Error::from)
    }

    async fn upsert_password_secret(
        &mut self,
        account_id: Uuid,
        secret: String,
    ) -> Result<AccountIdentity> {
        use schema::account_identities::dsl;

        let new_identity = NewAccountIdentity::password(account_id, secret.clone());
        diesel::insert_into(dsl::account_identities)
            .values(&new_identity)
            // A password identity already exists for this account: replace its
            // secret instead of failing the `(account_id, provider)` uniqueness.
            .on_conflict((dsl::account_id, dsl::provider))
            .do_update()
            .set(dsl::secret.eq(secret))
            .returning(AccountIdentity::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn delete_account_identity(
        &mut self,
        account_id: Uuid,
        provider: IdentityProvider,
    ) -> Result<DeleteIdentityOutcome> {
        use crate::AsyncConnection;

        // Lock and load the account's identity rows up front. Locking with a
        // plain `SELECT ... FOR UPDATE` (not `count(*) ... FOR UPDATE`, which
        // Postgres rejects as an aggregate with a locking clause) makes the
        // count-then-delete race-free: a concurrent delete of a sibling identity
        // cannot slip between the check and the delete and leave the account with
        // none.
        self.transaction(async |conn| {
            use schema::account_identities::dsl;

            let providers: Vec<IdentityProvider> = dsl::account_identities
                .filter(dsl::account_id.eq(account_id))
                .for_update()
                .select(dsl::provider)
                .load(conn)
                .await
                .map_err(Error::from)?;

            if !providers.contains(&provider) {
                return Ok(DeleteIdentityOutcome::NotFound);
            }
            // Refuse to remove the account's only identity: it must keep a way to
            // authenticate.
            if providers.len() <= 1 {
                return Ok(DeleteIdentityOutcome::LastIdentityKept);
            }

            diesel::delete(
                dsl::account_identities
                    .filter(dsl::account_id.eq(account_id))
                    .filter(dsl::provider.eq(provider)),
            )
            .execute(conn)
            .await
            .map_err(Error::from)?;

            Ok::<_, Error>(DeleteIdentityOutcome::Deleted)
        })
        .await
    }
}
