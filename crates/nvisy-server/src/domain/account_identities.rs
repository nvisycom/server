//! Account identity (credential) domain logic: list, set/change password, remove.
//!
//! Owns the credential rules a handler would otherwise inline — verifying the
//! current password before a change, strength-checking and hashing a new one, the
//! upsert-secret-and-stamp transaction, and the last-identity guard on removal.
//!
//! One authorization concern stays in the handler because it is transport, not
//! domain: obtaining a step-up re-authentication proof (an OIDC/NATS-KV round-trip)
//! before a *first* password is set. The handler consumes that proof and calls
//! [`set_first_password`](AccountIdentityService::set_first_password) with a
//! [`ReauthVerified`] marker; a change instead goes through
//! [`change_password`](AccountIdentityService::change_password), which verifies the
//! current password. The service reads [`has_password`](AccountIdentityService::has_password)
//! so the handler knows which path to take and consumes the single-use proof only
//! when one is actually required.

use nvisy_postgres::model::{AccountIdentity, UpdateAccount};
use nvisy_postgres::query::{AccountIdentityRepository, AccountRepository, DeleteIdentityOutcome};
use nvisy_postgres::types::IdentityProvider;
use nvisy_postgres::{AsyncConnection, PgClient};
use uuid::Uuid;

use crate::handler::utility::build_password_user_inputs;
use crate::response::{Error, ErrorKind, Result};
use crate::service::PasswordService;

/// Tracing target for account identity domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::account";

/// How the caller authorized setting a first password on an account that has no
/// password yet. A live session alone must not mint a durable new credential, so
/// the handler supplies a consumed step-up re-authentication proof; this marker
/// records that it did.
#[derive(Debug, Clone, Copy)]
pub struct ReauthVerified;

/// Lists, sets, and removes an account's sign-in identities.
///
/// Holds the Postgres client (own-connection-per-call) and the password service
/// (to strength-check, hash, and verify). Resolved per request from
/// [`ServiceState`](crate::service::ServiceState).
#[derive(Clone)]
pub struct AccountIdentityService {
    postgres: PgClient,
    password: PasswordService,
}

impl AccountIdentityService {
    /// Creates an [`AccountIdentityService`] over its clients.
    pub fn new(postgres: PgClient, password: PasswordService) -> Self {
        Self { postgres, password }
    }

    /// Lists the account's sign-in methods (its password and any linked providers).
    pub async fn list(&self, account_id: Uuid) -> Result<Vec<AccountIdentity>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn.list_account_identities(account_id).await?)
    }

    /// Whether the account already has a password identity.
    ///
    /// The handler reads this to decide the authorization it must obtain — a change
    /// requires the current password, a first-set requires a step-up proof — and
    /// to consume that single-use proof only when one is actually needed.
    pub async fn has_password(&self, account_id: Uuid) -> Result<bool> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .find_account_identity(account_id, IdentityProvider::Password)
            .await?
            .and_then(|identity| identity.secret)
            .is_some())
    }

    /// Changes an existing password after verifying the current one.
    ///
    /// Rejects a wrong (or absent) current password with a 401, so a hijacked
    /// session or CSRF cannot silently reset it. The new password is strength-
    /// checked against the account's own identifiers, then the secret upsert and
    /// the `password_changed_at` stamp commit together.
    pub async fn change_password(
        &self,
        account_id: Uuid,
        current_password: Option<&str>,
        new_password: &str,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let account = conn
            .find_account_by_id(account_id)
            .await?
            .ok_or_else(|| Error::not_found("account"))?;

        let secret = conn
            .find_account_identity(account_id, IdentityProvider::Password)
            .await?
            .and_then(|identity| identity.secret)
            .ok_or_else(|| {
                ErrorKind::Unauthorized
                    .with_message("Current password is incorrect")
                    .with_resource("account")
            })?;

        let verified =
            current_password.is_some_and(|current| self.password.verify(current, &secret).is_ok());
        if !verified {
            tracing::warn!(target: TRACING_TARGET, "Password change failed: current password incorrect");
            return Err(ErrorKind::Unauthorized
                .with_message("Current password is incorrect")
                .with_resource("account"));
        }

        // Release this connection before `write_password` acquires its own, so the
        // two are never held at once (which under load could deadlock the pool).
        drop(conn);
        self.write_password(account_id, &account, new_password)
            .await
    }

    /// Sets a first password on an account that has none, given a consumed step-up
    /// re-authentication proof.
    ///
    /// Refuses (409) if the account already has a password — that path is
    /// [`change_password`](Self::change_password), which verifies the current one.
    pub async fn set_first_password(
        &self,
        account_id: Uuid,
        new_password: &str,
        _reauth: ReauthVerified,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let account = conn
            .find_account_by_id(account_id)
            .await?
            .ok_or_else(|| Error::not_found("account"))?;

        let existing = conn
            .find_account_identity(account_id, IdentityProvider::Password)
            .await?
            .and_then(|identity| identity.secret);
        if existing.is_some() {
            return Err(ErrorKind::Conflict
                .with_message("Account already has a password; change it instead")
                .with_resource("account"));
        }

        // Release this connection before `write_password` acquires its own, so the
        // two are never held at once (which under load could deadlock the pool).
        drop(conn);
        self.write_password(account_id, &account, new_password)
            .await
    }

    /// Strength-checks and hashes `new_password` against the account's own
    /// identifiers, then upserts the secret and stamps `password_changed_at` in one
    /// transaction so a partial failure never leaves the two out of sync.
    async fn write_password(
        &self,
        account_id: Uuid,
        account: &nvisy_postgres::model::Account,
        new_password: &str,
    ) -> Result<()> {
        // Bind the strength check to the account's own fields so a password derived
        // from the username/email is rejected.
        let user_inputs = build_password_user_inputs(
            account.username.as_str(),
            account.display_name.as_deref(),
            &account.email_address,
        );
        let secret = self
            .password
            .validate_and_hash(new_password, &user_inputs)?;

        let mut conn = self.postgres.get_connection().await?;
        conn.transaction(async |conn| {
            conn.upsert_password_secret(account_id, secret).await?;
            conn.update_account(
                account_id,
                UpdateAccount {
                    password_changed_at: Some(jiff::Timestamp::now().into()),
                    ..Default::default()
                },
            )
            .await?;
            Ok::<_, Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Account password set");
        Ok(())
    }

    /// Removes an identity, mapping the last-identity and not-found outcomes to
    /// their errors. Shared by the password and provider deletes: an account may
    /// never lose its only sign-in method.
    pub async fn remove_identity(
        &self,
        account_id: Uuid,
        provider: IdentityProvider,
    ) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        match conn.delete_account_identity(account_id, provider).await? {
            DeleteIdentityOutcome::Deleted => {
                tracing::info!(target: TRACING_TARGET, provider = ?provider, "Identity removed");
                Ok(())
            }
            DeleteIdentityOutcome::LastIdentityKept => Err(ErrorKind::Conflict
                .with_message("Cannot remove your only sign-in method; add another first")
                .with_resource("account_identity")),
            DeleteIdentityOutcome::NotFound => Err(ErrorKind::NotFound
                .with_message("No such sign-in method on this account")
                .with_resource("account_identity")),
        }
    }
}
