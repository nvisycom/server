//! Account repository for managing user accounts.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{Account, NewAccount, UpdateAccount};
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

    /// Lists all active accounts with pagination support.
    ///
    /// Retrieves accounts ordered by creation time with most recent first.
    fn list_accounts(
        &mut self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Updates the account password and records the change timestamp.
    ///
    /// Sets a new password hash and updates the password_changed_at field.
    fn update_password(
        &mut self,
        account_id: Uuid,
        password_hash: String,
    ) -> impl Future<Output = PgResult<Account>> + Send;

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

    /// Finds accounts filtered by their verification status.
    ///
    /// Useful for finding unverified accounts that may need reminder emails.
    fn find_accounts_by_verification_status(
        &mut self,
        is_verified: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts filtered by their suspension status.
    ///
    /// Useful for administrative review of suspended accounts.
    fn find_accounts_by_suspension_status(
        &mut self,
        is_suspended: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts created within the last 30 days.
    ///
    /// Useful for onboarding analytics and new user tracking.
    fn find_recently_created_accounts(
        &mut self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts with no recent activity (no login in last 90 days).
    ///
    /// Useful for identifying dormant accounts for cleanup or re-engagement.
    fn find_inactive_accounts(
        &mut self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts registered with a specific email domain.
    ///
    /// Useful for organization-based filtering and analytics.
    fn find_accounts_by_domain(
        &mut self,
        domain: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;
}

impl AccountRepository for PgConnection {
    async fn create_account(&mut self, mut new_account: NewAccount) -> PgResult<Account> {
        use schema::accounts;

        // Normalize fields: trim whitespace
        new_account.display_name = new_account.display_name.trim().to_owned();
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

    async fn update_account(
        &mut self,
        account_id: Uuid,
        mut updates: UpdateAccount,
    ) -> PgResult<Account> {
        use schema::accounts::{self, dsl};

        // Normalize fields: trim whitespace
        if let Some(name) = updates.display_name.as_mut() {
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
        use schema::accounts::{self, dsl};

        diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .returning(Account::as_returning())
            .get_result(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn list_accounts(&mut self, pagination: Pagination) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn update_password(
        &mut self,
        account_id: Uuid,
        password_hash: String,
    ) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                password_hash: Some(password_hash),
                password_changed_at: Some(jiff_diesel::Timestamp::from(Timestamp::now())),
                ..Default::default()
            },
        )
        .await
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

    async fn find_accounts_by_verification_status(
        &mut self,
        is_verified: bool,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::is_verified.eq(is_verified))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_accounts_by_suspension_status(
        &mut self,
        is_suspended: bool,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::is_suspended.eq(is_suspended))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_recently_created_accounts(
        &mut self,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let thirty_days_ago = Timestamp::now() - Span::new().days(30);
        let thirty_days_ago = jiff_diesel::Timestamp::from(thirty_days_ago);

        accounts::table
            .filter(dsl::created_at.gt(thirty_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_inactive_accounts(&mut self, pagination: Pagination) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let ninety_days_ago = Timestamp::now() - Span::new().days(90);
        let ninety_days_ago = jiff_diesel::Timestamp::from(ninety_days_ago);

        accounts::table
            .filter(dsl::updated_at.lt(ninety_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_accounts_by_domain(
        &mut self,
        domain: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let domain_pattern = format!("%@{}", domain);

        accounts::table
            .filter(dsl::email_address.like(domain_pattern))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }
}
