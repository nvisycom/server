//! Account repository for managing user accounts.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use ipnet::IpNet;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{Account, NewAccount, UpdateAccount};
use crate::{PgClient, PgError, PgResult, schema};

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
        &self,
        new_account: NewAccount,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Finds an account by its unique identifier.
    ///
    /// Retrieves a specific account using its UUID, automatically excluding
    /// soft-deleted accounts.
    fn find_account_by_id(
        &self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Finds an account by email address.
    ///
    /// Retrieves an account using its email for authentication and lookup.
    /// Email comparison is case-insensitive.
    fn find_account_by_email(
        &self,
        email: &str,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Updates an account with new information.
    ///
    /// Applies partial updates to an existing account. Only fields set
    /// to `Some(value)` will be modified.
    fn update_account(
        &self,
        account_id: Uuid,
        updates: UpdateAccount,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Soft deletes an account by setting the deletion timestamp.
    ///
    /// Marks an account as deleted without permanently removing it,
    /// preserving data for audit purposes.
    fn delete_account(&self, account_id: Uuid) -> impl Future<Output = PgResult<Account>> + Send;

    /// Lists all active accounts with pagination support.
    ///
    /// Retrieves accounts ordered by creation time with most recent first.
    fn list_accounts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Records a failed login attempt and applies automatic locking if needed.
    ///
    /// Increments the failed login counter and locks the account for one hour
    /// after five failed attempts.
    fn record_failed_login(
        &self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Records a successful login and resets security counters.
    ///
    /// Clears failed login attempts and removes any account locks.
    fn record_successful_login(
        &self,
        account_id: Uuid,
        ip_address: IpNet,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Unlocks an account by clearing security locks and resetting counters.
    ///
    /// Removes any temporary locks and resets failed login attempts to zero.
    fn unlock_account(&self, account_id: Uuid) -> impl Future<Output = PgResult<Account>> + Send;

    /// Updates the account password and records the change timestamp.
    ///
    /// Sets a new password hash and updates the password_changed_at field.
    fn update_password(
        &self,
        account_id: Uuid,
        password_hash: String,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Verifies an account by setting the verification status to true.
    ///
    /// Typically called after email verification is complete.
    fn verify_account(&self, account_id: Uuid) -> impl Future<Output = PgResult<Account>> + Send;

    /// Suspends an account by setting the suspension status to true.
    ///
    /// Suspended accounts cannot authenticate or access resources.
    fn suspend_account(&self, account_id: Uuid) -> impl Future<Output = PgResult<Account>> + Send;

    /// Unsuspends an account by setting the suspension status to false.
    ///
    /// Restores normal access to a previously suspended account.
    fn unsuspend_account(&self, account_id: Uuid)
    -> impl Future<Output = PgResult<Account>> + Send;

    /// Checks if an email address is already registered in the system.
    ///
    /// Used during registration to prevent duplicate accounts.
    fn email_exists(&self, email: &str) -> impl Future<Output = PgResult<bool>> + Send;

    /// Finds accounts filtered by their verification status.
    ///
    /// Useful for finding unverified accounts that may need reminder emails.
    fn find_accounts_by_verification_status(
        &self,
        is_verified: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts filtered by their suspension status.
    ///
    /// Useful for administrative review of suspended accounts.
    fn find_accounts_by_suspension_status(
        &self,
        is_suspended: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts that are currently locked due to failed login attempts.
    ///
    /// Returns accounts with active locks ordered by lock expiration time.
    fn find_locked_accounts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts created within the last 30 days.
    ///
    /// Useful for onboarding analytics and new user tracking.
    fn find_recently_created_accounts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts with no recent activity (no login in last 90 days).
    ///
    /// Useful for identifying dormant accounts for cleanup or re-engagement.
    fn find_inactive_accounts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts registered with a specific email domain.
    ///
    /// Useful for organization-based filtering and analytics.
    fn find_accounts_by_domain(
        &self,
        domain: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts with high numbers of failed login attempts.
    ///
    /// Useful for security monitoring and identifying potential attacks.
    fn find_accounts_with_high_failed_attempts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Automatically unlocks accounts whose lock period has expired.
    ///
    /// Should be run periodically to restore access to locked accounts.
    fn unlock_expired_accounts(&self) -> impl Future<Output = PgResult<Vec<Account>>> + Send;
}

impl AccountRepository for PgClient {
    async fn create_account(&self, new_account: NewAccount) -> PgResult<Account> {
        use schema::accounts;

        let mut conn = self.get_connection().await?;

        diesel::insert_into(accounts::table)
            .values(&new_account)
            .returning(Account::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_account_by_id(&self, account_id: Uuid) -> PgResult<Option<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        accounts::table
            .filter(dsl::id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(Account::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn find_account_by_email(&self, email: &str) -> PgResult<Option<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        accounts::table
            .filter(dsl::email_address.eq(email.to_lowercase()))
            .filter(dsl::deleted_at.is_null())
            .select(Account::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn update_account(&self, account_id: Uuid, updates: UpdateAccount) -> PgResult<Account> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(&updates)
            .returning(Account::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn delete_account(&self, account_id: Uuid) -> PgResult<Account> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .returning(Account::as_returning())
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn list_accounts(&self, pagination: Pagination) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        accounts::table
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn record_failed_login(&self, account_id: Uuid) -> PgResult<Account> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        let account = diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(dsl::failed_login_attempts.eq(dsl::failed_login_attempts + 1))
            .returning(Account::as_returning())
            .get_result::<Account>(&mut conn)
            .await
            .map_err(PgError::from)?;

        if account.failed_login_attempts >= 5 {
            let lock_until = Timestamp::now() + Span::new().hours(1);
            let lock_until = jiff_diesel::Timestamp::from(lock_until);
            self.update_account(
                account_id,
                UpdateAccount {
                    locked_until: Some(lock_until),
                    ..Default::default()
                },
            )
            .await
        } else {
            Ok(account)
        }
    }

    async fn record_successful_login(
        &self,
        account_id: Uuid,
        _ip_address: IpNet,
    ) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                failed_login_attempts: Some(0),
                locked_until: None,
                ..Default::default()
            },
        )
        .await
    }

    async fn unlock_account(&self, account_id: Uuid) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                failed_login_attempts: Some(0),
                locked_until: None,
                ..Default::default()
            },
        )
        .await
    }

    async fn update_password(&self, account_id: Uuid, password_hash: String) -> PgResult<Account> {
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

    async fn verify_account(&self, account_id: Uuid) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                is_verified: Some(true),
                ..Default::default()
            },
        )
        .await
    }

    async fn suspend_account(&self, account_id: Uuid) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                is_suspended: Some(true),
                ..Default::default()
            },
        )
        .await
    }

    async fn unsuspend_account(&self, account_id: Uuid) -> PgResult<Account> {
        self.update_account(
            account_id,
            UpdateAccount {
                is_suspended: Some(false),
                ..Default::default()
            },
        )
        .await
    }

    async fn email_exists(&self, email: &str) -> PgResult<bool> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        let count: i64 = accounts::table
            .filter(dsl::email_address.eq(email.to_lowercase()))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(&mut conn)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }

    async fn find_accounts_by_verification_status(
        &self,
        is_verified: bool,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        accounts::table
            .filter(dsl::is_verified.eq(is_verified))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_accounts_by_suspension_status(
        &self,
        is_suspended: bool,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        accounts::table
            .filter(dsl::is_suspended.eq(is_suspended))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_locked_accounts(&self, pagination: Pagination) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        accounts::table
            .filter(dsl::locked_until.gt(jiff_diesel::Timestamp::from(Timestamp::now())))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::locked_until.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_recently_created_accounts(
        &self,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        let thirty_days_ago = Timestamp::now() - Span::new().days(30);
        let thirty_days_ago = jiff_diesel::Timestamp::from(thirty_days_ago);

        accounts::table
            .filter(dsl::created_at.gt(thirty_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_inactive_accounts(&self, pagination: Pagination) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        let ninety_days_ago = Timestamp::now() - Span::new().days(90);
        let ninety_days_ago = jiff_diesel::Timestamp::from(ninety_days_ago);

        accounts::table
            .filter(dsl::updated_at.lt(ninety_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_accounts_by_domain(
        &self,
        domain: &str,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        let domain_pattern = format!("%@{}", domain);

        accounts::table
            .filter(dsl::email_address.like(domain_pattern))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn find_accounts_with_high_failed_attempts(
        &self,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        accounts::table
            .filter(dsl::failed_login_attempts.ge(3))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::failed_login_attempts.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(&mut conn)
            .await
            .map_err(PgError::from)
    }

    async fn unlock_expired_accounts(&self) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let mut conn = self.get_connection().await?;

        diesel::update(
            accounts::table.filter(
                dsl::locked_until
                    .is_not_null()
                    .and(dsl::locked_until.le(jiff_diesel::Timestamp::from(Timestamp::now()))),
            ),
        )
        .set((
            dsl::locked_until.eq(None::<jiff_diesel::Timestamp>),
            dsl::failed_login_attempts.eq(0),
        ))
        .returning(Account::as_returning())
        .get_results(&mut conn)
        .await
        .map_err(PgError::from)
    }
}
