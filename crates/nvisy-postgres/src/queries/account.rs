//! Account repository for managing account database operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::models::{Account, NewAccount, UpdateAccount};
use crate::{PgError, PgResult, schema};

/// Repository for account-related database operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountRepository;

impl AccountRepository {
    /// Creates a new account repository instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new account in the database.
    pub async fn create_account(
        conn: &mut AsyncPgConnection,
        new_account: NewAccount,
    ) -> PgResult<Account> {
        use schema::accounts;

        diesel::insert_into(accounts::table)
            .values(&new_account)
            .returning(Account::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds an account by its ID.
    pub async fn find_account_by_id(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<Option<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .select(Account::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Finds an account by email address.
    pub async fn find_account_by_email(
        conn: &mut AsyncPgConnection,
        email: &str,
    ) -> PgResult<Option<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::email_address.eq(email.to_lowercase()))
            .filter(dsl::deleted_at.is_null())
            .select(Account::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Updates an account by ID.
    pub async fn update_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        updates: UpdateAccount,
    ) -> PgResult<Account> {
        use schema::accounts::{self, dsl};

        diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(&updates)
            .returning(Account::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Soft deletes an account by setting deleted_at timestamp.
    pub async fn delete_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<()> {
        use schema::accounts::{self, dsl};

        diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }

    /// Lists all accounts with pagination.
    pub async fn list_accounts(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    // Authentication helper methods

    /// Records a failed login attempt and potentially locks the account.
    pub async fn record_failed_login(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<Account> {
        use schema::accounts::{self, dsl};

        // Increment failed login attempts
        let account = diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(dsl::failed_login_attempts.eq(dsl::failed_login_attempts + 1))
            .returning(Account::as_returning())
            .get_result::<Account>(conn)
            .await
            .map_err(PgError::from)?;

        // Lock account if too many failed attempts
        if account.failed_login_attempts >= 5 {
            let lock_until = OffsetDateTime::now_utc() + time::Duration::hours(1);
            Self::update_account(
                conn,
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

    /// Records a successful login and resets failed login attempts.
    pub async fn record_successful_login(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        ip_address: IpNet,
    ) -> PgResult<Account> {
        let now = OffsetDateTime::now_utc();
        Self::update_account(
            conn,
            account_id,
            UpdateAccount {
                failed_login_attempts: Some(0),
                locked_until: None,
                last_login_at: Some(now),
                last_login_ip: Some(ip_address),
                ..Default::default()
            },
        )
        .await
    }

    /// Unlocks an account by clearing the locked_until timestamp.
    pub async fn unlock_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<Account> {
        Self::update_account(
            conn,
            account_id,
            UpdateAccount {
                failed_login_attempts: Some(0),
                locked_until: None,
                ..Default::default()
            },
        )
        .await
    }

    /// Updates the account password and records the change timestamp.
    pub async fn update_password(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        password_hash: String,
    ) -> PgResult<Account> {
        Self::update_account(
            conn,
            account_id,
            UpdateAccount {
                password_hash: Some(password_hash),
                password_changed_at: Some(OffsetDateTime::now_utc()),
                ..Default::default()
            },
        )
        .await
    }

    /// Verifies an account by setting is_verified to true.
    pub async fn verify_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<Account> {
        Self::update_account(
            conn,
            account_id,
            UpdateAccount {
                is_verified: Some(true),
                ..Default::default()
            },
        )
        .await
    }

    /// Suspends an account by setting is_suspended to true.
    pub async fn suspend_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<Account> {
        Self::update_account(
            conn,
            account_id,
            UpdateAccount {
                is_suspended: Some(true),
                ..Default::default()
            },
        )
        .await
    }

    /// Unsuspends an account by setting is_suspended to false.
    pub async fn unsuspend_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<Account> {
        Self::update_account(
            conn,
            account_id,
            UpdateAccount {
                is_suspended: Some(false),
                ..Default::default()
            },
        )
        .await
    }

    // Query methods

    /// Checks if an email address is already in use.
    pub async fn email_exists(
        conn: &mut AsyncPgConnection,
        email: &str,
    ) -> PgResult<bool> {
        use schema::accounts::{self, dsl};

        let count: i64 = accounts::table
            .filter(dsl::email_address.eq(email.to_lowercase()))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(count > 0)
    }

    /// Finds accounts by verification status.
    pub async fn find_accounts_by_verification_status(
        conn: &mut AsyncPgConnection,
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
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds accounts by suspension status.
    pub async fn find_accounts_by_suspension_status(
        conn: &mut AsyncPgConnection,
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
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds currently locked accounts.
    pub async fn find_locked_accounts(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::locked_until.gt(OffsetDateTime::now_utc()))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::locked_until.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds recently created accounts (within last 30 days).
    pub async fn find_recently_created_accounts(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let thirty_days_ago = OffsetDateTime::now_utc() - time::Duration::days(30);

        accounts::table
            .filter(dsl::created_at.gt(thirty_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds inactive accounts (no login in last 90 days).
    pub async fn find_inactive_accounts(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let ninety_days_ago = OffsetDateTime::now_utc() - time::Duration::days(90);

        accounts::table
            .filter(
                dsl::last_login_at
                    .is_null()
                    .or(dsl::last_login_at.lt(ninety_days_ago)),
            )
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds accounts by email domain.
    pub async fn find_accounts_by_domain(
        conn: &mut AsyncPgConnection,
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
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds accounts with high failed login attempts.
    pub async fn find_accounts_with_high_failed_attempts(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        accounts::table
            .filter(dsl::failed_login_attempts.ge(3))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::failed_login_attempts.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    // Statistics and maintenance

    /// Gets comprehensive statistics about accounts.
    pub async fn get_account_statistics(
        conn: &mut AsyncPgConnection,
    ) -> PgResult<AccountStatistics> {
        use schema::accounts::{self, dsl};

        let now = OffsetDateTime::now_utc();
        let thirty_days_ago = now - time::Duration::days(30);

        // Total count
        let total_count: i64 = accounts::table
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Verified count
        let verified_count: i64 = accounts::table
            .filter(dsl::is_verified.eq(true))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Suspended count
        let suspended_count: i64 = accounts::table
            .filter(dsl::is_suspended.eq(true))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Admin count
        let admin_count: i64 = accounts::table
            .filter(dsl::is_admin.eq(true))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Locked count
        let locked_count: i64 = accounts::table
            .filter(dsl::locked_until.gt(now))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Recent count (created in last 30 days)
        let recent_count: i64 = accounts::table
            .filter(dsl::created_at.gt(thirty_days_ago))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        // Active count (logged in within last 30 days)
        let active_count: i64 = accounts::table
            .filter(dsl::last_login_at.gt(thirty_days_ago))
            .filter(dsl::deleted_at.is_null())
            .count()
            .get_result(conn)
            .await
            .map_err(PgError::from)?;

        Ok(AccountStatistics {
            total_count,
            verified_count,
            suspended_count,
            admin_count,
            locked_count,
            recent_count,
            active_count,
        })
    }

    /// Unlocks expired account locks.
    pub async fn unlock_expired_accounts(
        conn: &mut AsyncPgConnection,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

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
        .get_results(conn)
        .await
        .map_err(PgError::from)
    }
}

/// Statistics for accounts.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountStatistics {
    /// Total number of accounts
    pub total_count: i64,
    /// Number of verified accounts
    pub verified_count: i64,
    /// Number of suspended accounts
    pub suspended_count: i64,
    /// Number of admin accounts
    pub admin_count: i64,
    /// Number of currently locked accounts
    pub locked_count: i64,
    /// Number of accounts created in last 30 days
    pub recent_count: i64,
    /// Number of accounts active in last 30 days
    pub active_count: i64,
}

impl AccountStatistics {
    /// Returns the verification rate as a percentage (0-100).
    pub fn verification_rate(&self) -> f64 {
        if self.total_count == 0 {
            100.0
        } else {
            (self.verified_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Returns the suspension rate as a percentage (0-100).
    pub fn suspension_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            (self.suspended_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Returns the activity rate as a percentage (0-100).
    pub fn activity_rate(&self) -> f64 {
        if self.total_count == 0 {
            100.0
        } else {
            (self.active_count as f64 / self.total_count as f64) * 100.0
        }
    }

    /// Returns the growth rate (new accounts in last 30 days) as a percentage.
    pub fn growth_rate(&self) -> f64 {
        if self.total_count == 0 {
            0.0
        } else {
            (self.recent_count as f64 / self.total_count as f64) * 100.0
        }
    }
}
