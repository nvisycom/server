//! Account identity repository for managing authentication methods.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{AccountIdentity, NewAccountIdentity};
use crate::types::IdentityProvider;
use crate::{DieselError, Error, JiffTimestamp, PgConnection, Result, schema};

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
    /// its stable subject (`sub`) claim. Each provider is pinned to a single
    /// issuer, so the subject is unique within the provider.
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

    /// Links an OIDC identity to an account, tolerating a concurrent link of the
    /// *same* provider account.
    ///
    /// A caller checks that the account has no identity for this provider before
    /// linking, but a concurrent callback for the same person can slip an insert
    /// into that window and trip the `(account_id, provider)` uniqueness. Rather
    /// than surface that race as an error, this resolves it by what actually
    /// landed: the same subject means the link is already done ([`AlreadyLinked`]);
    /// a *different* provider account means the slot is taken ([`ProviderConflict`]).
    ///
    /// The `identity` must be an OIDC identity (its `provider_subject` set); a
    /// password identity is a caller bug and yields [`ProviderConflict`] rather
    /// than matching.
    ///
    /// [`AlreadyLinked`]: LinkIdentityOutcome::AlreadyLinked
    /// [`ProviderConflict`]: LinkIdentityOutcome::ProviderConflict
    fn link_oidc_identity(
        &mut self,
        identity: NewAccountIdentity,
    ) -> impl Future<Output = Result<LinkIdentityOutcome>> + Send;

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

/// The result of a [`link_oidc_identity`](AccountIdentityRepository::link_oidc_identity)
/// call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkIdentityOutcome {
    /// The identity was linked by this call.
    Linked,
    /// The account already had this exact identity (same subject), so the link is
    /// a no-op — a concurrent callback did it first.
    AlreadyLinked,
    /// The account already has a *different* identity for this provider; the slot
    /// is taken and the caller should treat it as a conflict.
    ProviderConflict,
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

    async fn link_oidc_identity(
        &mut self,
        identity: NewAccountIdentity,
    ) -> Result<LinkIdentityOutcome> {
        use crate::AsyncConnection;
        use crate::types::{AccountIdentityConstraints, ConstraintViolation};

        let account_id = identity.account_id;
        let provider = identity.provider;
        let subject = identity.provider_subject.clone();

        // Lock the account row and reject a tombstoned account before writing, so
        // an identity insert cannot race a concurrent `delete_account` (which
        // soft-deletes the account and clears its identities in one transaction)
        // and leave a dead account holding a live identity — which would also pin
        // the `(provider, provider_subject)` uniqueness against a deleted account.
        self.transaction(async |conn| {
            lock_active_account(conn, account_id).await?;

            // Run the insert in a savepoint (a nested `transaction`). A unique
            // violation aborts only the savepoint, not the outer transaction, so
            // the follow-up read below can still run — a bare insert here would
            // poison the whole transaction on `23505` and the read would fail with
            // `25P02` instead of resolving the conflict.
            let insert = conn
                .transaction(async |conn| conn.create_account_identity(identity).await)
                .await;

            match insert {
                Ok(_) => Ok(LinkIdentityOutcome::Linked),
                // A concurrent callback won the `(account_id, provider)` slot.
                // Whether that is benign depends on what it linked, so read the
                // winning row back and compare.
                Err(err)
                    if matches!(
                        err.constraint_violation(),
                        Some(ConstraintViolation::AccountIdentity(
                            AccountIdentityConstraints::AccountProviderUnique
                        ))
                    ) =>
                {
                    let existing = conn.find_account_identity(account_id, provider).await?;
                    let same = existing.is_some_and(|row| {
                        // Only an OIDC identity can match; a password row (subject
                        // `None`) never equals a link attempt.
                        row.provider_subject.is_some() && row.provider_subject == subject
                    });
                    Ok(if same {
                        LinkIdentityOutcome::AlreadyLinked
                    } else {
                        LinkIdentityOutcome::ProviderConflict
                    })
                }
                Err(err) => Err(err),
            }
        })
        .await
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

        use crate::AsyncConnection;

        // Lock the account row and reject a tombstoned account before writing, so
        // setting a password cannot race a concurrent `delete_account` and leave
        // a dead account holding a live credential (see `link_oidc_identity`).
        self.transaction(async |conn| {
            lock_active_account(conn, account_id).await?;

            let new_identity = NewAccountIdentity::password(account_id, secret.clone());
            diesel::insert_into(dsl::account_identities)
                .values(&new_identity)
                // A password identity already exists for this account: replace its
                // secret instead of failing the `(account_id, provider)` uniqueness.
                .on_conflict((dsl::account_id, dsl::provider))
                .do_update()
                .set(dsl::secret.eq(secret))
                .returning(AccountIdentity::as_returning())
                .get_result(conn)
                .await
                .map_err(Error::from)
        })
        .await
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

/// Locks the account row `FOR UPDATE` and confirms it is live, returning a
/// not-found error if the account is absent or soft-deleted (`deleted_at` set).
///
/// Called inside an identity-write transaction so the write serializes against a
/// concurrent [`delete_account`](super::AccountRepository::delete_account): the
/// lock forces the delete's `UPDATE accounts … SET deleted_at` to commit before
/// this sees the row, and the `deleted_at` recheck then rejects the write against
/// a tombstoned account.
async fn lock_active_account(conn: &mut PgConnection, account_id: Uuid) -> Result<()> {
    use schema::accounts::{self, dsl};

    // The outer `Option` is row presence; the inner is the nullable `deleted_at`.
    let row: Option<Option<JiffTimestamp>> = accounts::table
        .filter(dsl::id.eq(account_id))
        .for_update()
        .select(dsl::deleted_at)
        .first::<Option<JiffTimestamp>>(conn)
        .await
        .optional()
        .map_err(Error::from)?;

    match row {
        // Row exists and is not tombstoned (`deleted_at IS NULL`).
        Some(None) => Ok(()),
        // Absent, or soft-deleted: treat as gone.
        _ => Err(Error::from(DieselError::NotFound)),
    }
}

#[cfg(test)]
mod tests {
    use super::{DeleteIdentityOutcome, LinkIdentityOutcome};
    use crate::model::NewAccountIdentity;
    use crate::query::AccountIdentityRepository;
    use crate::test_util::TestDatabase;
    use crate::types::IdentityProvider;

    /// A password-shaped secret at least 60 chars long, to satisfy the
    /// `account_identities_password_shape` check.
    const SECRET: &str =
        "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHR2YWx1ZQ$aaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SECRET_ALT: &str =
        "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHR2YWx1ZQ$bbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[tokio::test]
    async fn link_oidc_reports_linked_already_linked_and_conflict() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let identity = || {
            NewAccountIdentity::oidc(
                account_id,
                IdentityProvider::Google,
                "google-subject-1".to_owned(),
                Some("user@example.com".to_owned()),
            )
        };

        // First link succeeds.
        assert_eq!(
            conn.link_oidc_identity(identity()).await?,
            LinkIdentityOutcome::Linked
        );

        // Re-linking the SAME provider subject is benign (idempotent) — the
        // conflict-recovery read runs cleanly inside the savepoint.
        assert_eq!(
            conn.link_oidc_identity(identity()).await?,
            LinkIdentityOutcome::AlreadyLinked
        );

        // Linking the same provider with a DIFFERENT subject is a conflict: the
        // provider slot is already taken by another subject.
        let other = NewAccountIdentity::oidc(
            account_id,
            IdentityProvider::Google,
            "google-subject-2".to_owned(),
            None,
        );
        assert_eq!(
            conn.link_oidc_identity(other).await?,
            LinkIdentityOutcome::ProviderConflict
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_refuses_the_last_identity() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        // Give the account two identities: a password and a Google link.
        let _ = conn
            .upsert_password_secret(account_id, SECRET.to_owned())
            .await?;
        conn.link_oidc_identity(NewAccountIdentity::oidc(
            account_id,
            IdentityProvider::Google,
            "sub".to_owned(),
            None,
        ))
        .await?;

        // Deleting one of two succeeds.
        assert_eq!(
            conn.delete_account_identity(account_id, IdentityProvider::Google)
                .await?,
            DeleteIdentityOutcome::Deleted
        );
        // Deleting the now-only identity is refused.
        assert_eq!(
            conn.delete_account_identity(account_id, IdentityProvider::Password)
                .await?,
            DeleteIdentityOutcome::LastIdentityKept
        );
        // Deleting a provider the account never had is NotFound.
        assert_eq!(
            conn.delete_account_identity(account_id, IdentityProvider::Microsoft)
                .await?,
            DeleteIdentityOutcome::NotFound
        );
        Ok(())
    }

    #[tokio::test]
    async fn upsert_password_replaces_the_existing_secret() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let mut conn = db.client.get_connection().await?;

        let first = conn
            .upsert_password_secret(account_id, SECRET.to_owned())
            .await?;
        assert_eq!(first.secret.as_deref(), Some(SECRET));

        // A second upsert replaces the secret rather than inserting a duplicate.
        let second = conn
            .upsert_password_secret(account_id, SECRET_ALT.to_owned())
            .await?;
        assert_eq!(second.secret.as_deref(), Some(SECRET_ALT));

        // Still exactly one identity (the password), not two.
        let identities = conn.list_account_identities(account_id).await?;
        assert_eq!(identities.len(), 1);
        assert_eq!(identities[0].provider, IdentityProvider::Password);
        Ok(())
    }
}
