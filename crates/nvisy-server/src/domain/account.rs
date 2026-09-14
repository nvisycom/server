//! Account profile domain logic: read, read-public, update, delete.
//!
//! Owns the self-service profile rules a handler would otherwise inline — the
//! shared-workspace visibility gate for a public profile and the email/username
//! uniqueness checks on update. Account CRUD carries no workspace events, so there
//! is no event emission here. Credentials (password, linked providers) are managed
//! by [`AccountIdentityService`], not here, and avatar upload/delete stay in the
//! handler over [`AvatarService`].
//!
//! [`AccountIdentityService`]: super::AccountIdentityService
//! [`AvatarService`]: crate::service::AvatarService

use nvisy_postgres::PgClient;
use nvisy_postgres::model::{Account, UpdateAccount};
use nvisy_postgres::query::{AccountRepository, WorkspaceMemberRepository};
use uuid::Uuid;

use crate::response::{Error, ErrorKind, Result};

/// Tracing target for account domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::account";

/// Reads, updates, and deletes accounts.
///
/// Holds the Postgres client and acquires its own connection per call, so each
/// mutation is a self-contained transaction. Resolved per request from
/// [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct AccountService {
    postgres: PgClient,
}

impl AccountService {
    /// Creates an [`AccountService`] over the given connection pool.
    #[must_use]
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// Finds an account by id, or a `NotFound`.
    pub async fn find(&self, account_id: Uuid) -> Result<Account> {
        let mut conn = self.postgres.get_connection().await?;
        conn.find_account_by_id(account_id)
            .await?
            .ok_or_else(|| Error::not_found("account"))
    }

    /// Finds another account's profile by its id, visible only to a requester
    /// that shares a workspace with it.
    ///
    /// A non-shared (or non-existent) account is reported as not-found rather than
    /// forbidden, so this endpoint cannot be used to distinguish existing from
    /// non-existing accounts.
    pub async fn find_public(&self, requester_id: Uuid, account_id: Uuid) -> Result<Account> {
        let mut conn = self.postgres.get_connection().await?;

        let account = conn
            .find_account_by_id(account_id)
            .await?
            .ok_or_else(|| Error::not_found("account"))?;

        let shares_workspace = conn
            .accounts_share_workspace(requester_id, account.id)
            .await?;
        if !shares_workspace {
            tracing::warn!(target: TRACING_TARGET, "Account not accessible: no shared workspace");
            return Err(Error::not_found("account"));
        }

        Ok(account)
    }

    /// Updates an account's profile, enforcing email and username uniqueness, and
    /// returns the updated account.
    ///
    /// A conflicting email or handle is a 409; the check excludes the account's own
    /// row so re-submitting its current values is not a conflict.
    pub async fn update(&self, account_id: Uuid, updates: UpdateAccount) -> Result<Account> {
        let mut conn = self.postgres.get_connection().await?;

        if let Some(email) = &updates.email_address
            && conn.email_exists_for_other(email, account_id).await?
        {
            tracing::warn!(target: TRACING_TARGET, "Account update failed: email already exists");
            return Err(ErrorKind::Conflict.with_message("Email is already registered"));
        }

        if let Some(username) = &updates.username
            && conn.username_exists_for_other(username, account_id).await?
        {
            tracing::warn!(target: TRACING_TARGET, "Account update failed: username already taken");
            return Err(ErrorKind::Conflict.with_message("Handle is already taken"));
        }

        let account = conn.update_account(account_id, updates).await?;
        tracing::info!(target: TRACING_TARGET, "Account updated");
        Ok(account)
    }

    /// Soft-deletes an account, or a `NotFound` if it does not exist.
    pub async fn delete(&self, account_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        conn.delete_account(account_id)
            .await?
            .ok_or_else(|| Error::not_found("account"))?;
        tracing::info!(target: TRACING_TARGET, "Account deleted");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::model;
    use nvisy_postgres::test_util::TestDatabase;

    use super::*;

    #[tokio::test]
    async fn find_returns_the_account() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let service = AccountService::new(db.client.clone());

        let account = service.find(account_id).await?;
        assert_eq!(account.id, account_id);
        Ok(())
    }

    #[tokio::test]
    async fn update_changes_the_display_name() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let service = AccountService::new(db.client.clone());

        let updates = model::UpdateAccount {
            display_name: Some(Some("Renamed".to_owned())),
            ..Default::default()
        };
        let updated_account = service.update(account_id, updates).await?;
        assert_eq!(updated_account.display_name.as_deref(), Some("Renamed"));
        Ok(())
    }

    #[tokio::test]
    async fn delete_then_find_is_not_found() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let account_id = db.seed_account().await;
        let service = AccountService::new(db.client.clone());

        service.delete(account_id).await?;
        assert!(service.find(account_id).await.is_err());
        Ok(())
    }
}
