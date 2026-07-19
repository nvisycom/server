//! Account repository for managing user accounts.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{Account, NewAccount, UpdateAccount};
use crate::types::Username;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for account database operations.
///
/// Handles account lifecycle management including authentication, profile management,
/// and security operations.
pub trait AccountRepository {
    /// Creates a new user account with complete profile information.
    ///
    /// Inserts a new account record into the database with the provided
    /// details including email, password hash, and profile information.
    fn create_account(
        &mut self,
        new_account: NewAccount,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Finds an account by its unique identifier.
    ///
    /// Retrieves a specific account using its UUID, automatically excluding
    /// soft-deleted accounts.
    fn find_account_by_id(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Finds an account by email address.
    ///
    /// Retrieves an account using its email for authentication and lookup.
    /// Email comparison is case-insensitive.
    fn find_account_by_email(
        &mut self,
        email: &str,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Finds an account by its public handle.
    ///
    /// Retrieves an account using its username, excluding soft-deleted
    /// accounts. Comparison is case-insensitive.
    fn find_account_by_username(
        &mut self,
        username: &Username,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Finds an account by either email address or username.
    ///
    /// The identifier is treated as an email when it contains `@` (usernames
    /// never do), and as a username otherwise. Used to authenticate with
    /// either credential. Comparison is case-insensitive.
    fn find_account_by_identifier(
        &mut self,
        identifier: &str,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Updates an account with new information.
    ///
    /// Applies partial updates to an existing account. Only fields set
    /// to `Some(value)` will be modified.
    fn update_account(
        &mut self,
        account_id: Uuid,
        updates: UpdateAccount,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Soft deletes an account by setting the deletion timestamp.
    ///
    /// Marks an account as deleted without permanently removing it,
    /// preserving data for audit purposes. Returns `None` if the account
    /// was not found.
    fn delete_account(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Verifies an account by setting the verification status to true.
    ///
    /// Typically called after email verification is complete.
    fn verify_account(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Suspends an account by setting the suspension status to true.
    ///
    /// Suspended accounts cannot authenticate or access resources.
    fn suspend_account(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Unsuspends an account by setting the suspension status to false.
    ///
    /// Restores normal access to a previously suspended account.
    fn unsuspend_account(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Checks if an email address is already registered in the system.
    ///
    /// Used during registration to prevent duplicate accounts.
    fn email_exists(&mut self, email: &str) -> impl Future<Output = PgResult<bool>> + Send;

    /// Checks if an email address is used by another account.
    ///
    /// Used during account updates to prevent duplicate emails.
    fn email_exists_for_other(
        &mut self,
        email: &str,
        exclude_account_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Checks if a username is already registered in the system.
    ///
    /// Used during registration to prevent duplicate handles.
    fn username_exists(
        &mut self,
        username: &Username,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Checks if a username is used by another account.
    ///
    /// Used during account updates to prevent duplicate handles.
    fn username_exists_for_other(
        &mut self,
        username: &Username,
        exclude_account_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;
}

impl AccountRepository for PgConnection {
    async fn create_account(&mut self, mut new_account: NewAccount) -> PgResult<Account> {
        use schema::accounts;

        // Normalize fields: trim whitespace
        if let Some(ref mut name) = new_account.display_name {
            *name = name.trim().to_owned();
        }
        new_account.email_address = new_account.email_address.trim().to_lowercase();
        if let Some(ref mut company) = new_account.company_name {
            *company = company.trim().to_owned();
        }

        diesel::insert_into(accounts::table)
            .values(&new_account)
            .returning(Account::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_account_by_id(&mut self, account_id: Uuid) -> PgResult<Option<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(Account::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn find_account_by_email(&mut self, email: &str) -> PgResult<Option<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::email_address.eq(email.trim().to_lowercase()))
            .filter(dsl::deleted_at.is_null())
            .select(Account::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn find_account_by_username(&mut self, username: &Username) -> PgResult<Option<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::username.eq(username.as_str()))
            .filter(dsl::deleted_at.is_null())
            .select(Account::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn find_account_by_identifier(&mut self, identifier: &str) -> PgResult<Option<Account>> {
        if identifier.contains('@') {
            return self.find_account_by_email(identifier).await;
        }

        // An identifier that is not a well-formed username cannot match any
        // account. Still issue a lookup (guaranteed to miss) so every branch
        // performs one query and the login flow's timing stays uniform,
        // regardless of whether the identifier was syntactically valid.
        match Username::parse(identifier.trim()) {
            Ok(username) => self.find_account_by_username(&username).await,
            Err(_) => self.find_account_by_email(identifier).await,
        }
    }

    async fn update_account(
        &mut self,
        account_id: Uuid,
        mut updates: UpdateAccount,
    ) -> PgResult<Account> {
        use schema::accounts::{self, dsl};

        // Normalize fields: trim whitespace
        // Some(None) clears, Some(Some(value)) sets, None skips
        if let Some(Some(name)) = updates.display_name.as_mut() {
            *name = name.trim().to_owned();
        }
        if let Some(email) = updates.email_address.as_mut() {
            *email = email.trim().to_lowercase();
        }
        // Some(None) clears, Some(Some(value)) sets, None skips
        updates.company_name = updates
            .company_name
            .map(|opt| opt.map(|c| c.trim().to_owned()).filter(|c| !c.is_empty()));

        diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(&updates)
            .returning(Account::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn delete_account(&mut self, account_id: Uuid) -> PgResult<Option<Account>> {
        use diesel::dsl::now;
        use schema::accounts::{self, dsl};

        diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(dsl::deleted_at.eq(now))
            .returning(Account::as_returning())
            .get_result(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn verify_account(&mut self, account_id: Uuid) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                is_verified: Some(true),
                ..Default::default()
            },
        )
        .await
    }

    async fn suspend_account(&mut self, account_id: Uuid) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                is_suspended: Some(true),
                ..Default::default()
            },
        )
        .await
    }

    async fn unsuspend_account(&mut self, account_id: Uuid) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                is_suspended: Some(false),
                ..Default::default()
            },
        )
        .await
    }

    async fn email_exists(&mut self, email: &str) -> PgResult<bool> {
        use schema::accounts::{self, dsl};

        let count: i64 = accounts::table
            .filter(dsl::email_address.eq(email.trim().to_lowercase()))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }

    async fn email_exists_for_other(
        &mut self,
        email: &str,
        exclude_account_id: Uuid,
    ) -> PgResult<bool> {
        use schema::accounts::{self, dsl};

        let count: i64 = accounts::table
            .filter(dsl::email_address.eq(email.trim().to_lowercase()))
            .filter(dsl::id.ne(exclude_account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }

    async fn username_exists(&mut self, username: &Username) -> PgResult<bool> {
        use schema::accounts::{self, dsl};

        let count: i64 = accounts::table
            .filter(dsl::username.eq(username.as_str()))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }

    async fn username_exists_for_other(
        &mut self,
        username: &Username,
        exclude_account_id: Uuid,
    ) -> PgResult<bool> {
        use schema::accounts::{self, dsl};

        let count: i64 = accounts::table
            .filter(dsl::username.eq(username.as_str()))
            .filter(dsl::id.ne(exclude_account_id))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }
}
