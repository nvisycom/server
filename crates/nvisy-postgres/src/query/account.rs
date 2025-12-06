//! Account repository for managing account database operations.
//!
//! This module provides comprehensive database operations for user account management,
//! including authentication, profile management, security operations, and account
//! lifecycle management. It serves as the primary interface for all account-related
//! database interactions.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{Account, NewAccount, UpdateAccount};
use crate::{PgClient, PgError, PgResult, schema};

/// Repository for comprehensive account database operations.
///
/// Provides a complete set of database operations for managing user accounts throughout
/// their lifecycle. This repository handles authentication, profile management, security
/// features, and administrative operations with proper error handling and transaction support.
pub trait AccountRepository {
    /// Creates a new user account with complete profile information.
    fn create_account(
        &self,
        new_account: NewAccount,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Finds an account by its unique identifier.
    fn find_account_by_id(
        &self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Finds an account by email address for authentication and lookup.
    fn find_account_by_email(
        &self,
        email: &str,
    ) -> impl Future<Output = PgResult<Option<Account>>> + Send;

    /// Updates an existing account with new information.
    fn update_account(
        &self,
        account_id: Uuid,
        updates: UpdateAccount,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Soft deletes an account by setting the deletion timestamp.
    fn delete_account(&self, account_id: Uuid) -> impl Future<Output = PgResult<Account>> + Send;

    /// Lists all active accounts with pagination support.
    fn list_accounts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Records a failed login attempt and applies automatic account locking if needed.
    fn record_failed_login(
        &self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Records a successful login and resets security counters.
    fn record_successful_login(
        &self,
        account_id: Uuid,
        _ip_address: IpNet,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Unlocks an account by clearing security locks and resetting counters.
    fn unlock_account(&self, account_id: Uuid) -> impl Future<Output = PgResult<Account>> + Send;

    /// Updates the account password and records the change timestamp.
    fn update_password(
        &self,
        account_id: Uuid,
        password_hash: String,
    ) -> impl Future<Output = PgResult<Account>> + Send;

    /// Verifies an account by setting the verification status to true.
    fn verify_account(&self, account_id: Uuid) -> impl Future<Output = PgResult<Account>> + Send;

    /// Suspends an account by setting the suspension status to true.
    fn suspend_account(&self, account_id: Uuid) -> impl Future<Output = PgResult<Account>> + Send;

    /// Unsuspends an account by setting the suspension status to false.
    fn unsuspend_account(&self, account_id: Uuid)
    -> impl Future<Output = PgResult<Account>> + Send;

    /// Checks if an email address is already registered in the system.
    fn email_exists(&self, email: &str) -> impl Future<Output = PgResult<bool>> + Send;

    /// Finds accounts filtered by their verification status.
    fn find_accounts_by_verification_status(
        &self,
        is_verified: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts filtered by their suspension status.
    fn find_accounts_by_suspension_status(
        &self,
        is_suspended: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts that are currently locked due to failed login attempts.
    fn find_locked_accounts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts created within the last 30 days.
    fn find_recently_created_accounts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts with no recent activity (no login in last 90 days).
    fn find_inactive_accounts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts registered with a specific email domain.
    fn find_accounts_by_domain(
        &self,
        domain: &str,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Finds accounts with high numbers of failed login attempts.
    fn find_accounts_with_high_failed_attempts(
        &self,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<Account>>> + Send;

    /// Automatically unlocks accounts whose lock period has expired.
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
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
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

        // Increment failed login attempts
        let account = diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(dsl::failed_login_attempts.eq(dsl::failed_login_attempts + 1))
            .returning(Account::as_returning())
            .get_result::<Account>(&mut conn)
            .await
            .map_err(PgError::from)?;

        // Lock account if too many failed attempts
        if account.failed_login_attempts >= 5 {
            let lock_until = OffsetDateTime::now_utc() + time::Duration::hours(1);
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
        let _now = OffsetDateTime::now_utc();
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
                password_changed_at: Some(OffsetDateTime::now_utc()),
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
            .filter(dsl::locked_until.gt(OffsetDateTime::now_utc()))
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

        let thirty_days_ago = OffsetDateTime::now_utc() - time::Duration::days(30);

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

        let ninety_days_ago = OffsetDateTime::now_utc() - time::Duration::days(90);

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
                    .and(dsl::locked_until.le(OffsetDateTime::now_utc())),
            ),
        )
        .set((
            dsl::locked_until.eq(None::<OffsetDateTime>),
            dsl::failed_login_attempts.eq(0),
        ))
        .returning(Account::as_returning())
        .get_results(&mut conn)
        .await
        .map_err(PgError::from)
    }
}
