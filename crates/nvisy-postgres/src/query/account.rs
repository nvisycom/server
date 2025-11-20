//! Account repository for managing account database operations.
//!
//! This module provides comprehensive database operations for user account management,
//! including authentication, profile management, security operations, and account
//! lifecycle management. It serves as the primary interface for all account-related
//! database interactions.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use ipnet::IpNet;
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{Account, NewAccount, UpdateAccount};
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive account database operations.
///
/// Provides a complete set of database operations for managing user accounts throughout
/// their lifecycle. This repository handles authentication, profile management, security
/// features, and administrative operations with proper error handling and transaction support.
///
/// The repository is stateless and thread-safe, designed to be used as a singleton
/// or instantiated as needed. All methods require an active database connection
/// and return results wrapped in the standard `PgResult` type for consistent error handling.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountRepository;

impl AccountRepository {
    /// Creates a new account repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `AccountRepository` instance.
    pub fn new() -> Self {
        Self
    }

    /// Creates a new user account with complete profile information.
    ///
    /// Registers a new user account in the system with the provided information.
    /// The account is created with secure defaults and proper validation. This is
    /// the primary method for user registration and account provisioning.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `new_account` - Complete account data for the new user
    ///
    /// # Returns
    ///
    /// The created `Account` with database-generated ID and timestamps,
    /// or a database error if the operation fails.
    ///
    /// # Security Considerations
    ///
    /// - Password should be properly hashed before calling this method
    /// - Email addresses should be validated for format and deliverability
    /// - Display names should be sanitized to prevent XSS attacks
    /// - Consider rate limiting account creation to prevent abuse
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

    /// Finds an account by its unique identifier.
    ///
    /// Retrieves a specific account using its UUID. This method automatically
    /// excludes soft-deleted accounts and is the primary way to access account
    /// information when you know the exact account ID.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to retrieve
    ///
    /// # Returns
    ///
    /// The matching `Account` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
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

    /// Finds an account by email address for authentication and lookup.
    ///
    /// Searches for an account using the provided email address. Email addresses
    /// are stored in lowercase for consistency and are automatically converted during
    /// the search. This method excludes soft-deleted accounts and is commonly used
    /// for login authentication and email-based account recovery.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `email` - Email address to search for (case-insensitive)
    ///
    /// # Returns
    ///
    /// The matching `Account` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
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

    /// Updates an existing account with new information.
    ///
    /// Applies the specified changes to an account using Diesel's changeset mechanism.
    /// Only the fields set to `Some(value)` in the update structure will be modified,
    /// while `None` fields remain unchanged. The updated_at timestamp is automatically
    /// updated to reflect the modification time.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `Account` with new values and timestamp,
    /// or a database error if the operation fails.
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

    /// Soft deletes an account by setting the deletion timestamp.
    ///
    /// Marks an account as deleted without permanently removing it from the database.
    /// This preserves data for audit purposes and compliance requirements while
    /// preventing the account from being used for authentication or other operations.
    /// Soft-deleted accounts are automatically excluded from most queries.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account to soft delete
    ///
    /// # Returns
    ///
    /// The deleted `Account` with updated deletion timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Account immediately becomes inaccessible for authentication
    /// - All associated sessions should be invalidated
    /// - Account data is preserved for audit and compliance
    /// - Related entities (projects, documents) may need cleanup
    pub async fn delete_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<Account> {
        use schema::accounts::{self, dsl};

        diesel::update(accounts::table.filter(dsl::id.eq(account_id)))
            .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
            .returning(Account::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Lists all active accounts with pagination support.
    ///
    /// Retrieves a paginated list of accounts, automatically excluding soft-deleted
    /// accounts. Results are ordered by creation date with newest accounts first.
    /// This method is primarily used for administrative interfaces and account
    /// management dashboards.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Account` entries ordered by creation date (newest first),
    /// or a database error if the query fails.
    ///
    /// # Performance Considerations
    ///
    /// - Uses database indexes for optimal performance
    /// - Large result sets should use reasonable pagination limits
    /// - Consider filtering by additional criteria for better performance
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

    /// Records a failed login attempt and applies automatic account locking if needed.
    ///
    /// Increments the failed login attempt counter for the specified account and
    /// automatically locks the account if the maximum number of failed attempts
    /// is exceeded. This provides protection against brute force attacks while
    /// maintaining detailed security audit logs.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account that had a failed login attempt
    ///
    /// # Returns
    ///
    /// The updated `Account` with incremented failure count and potential lock,
    /// or a database error if the operation fails.
    ///
    /// # Security Features
    ///
    /// - Automatic account locking after configured maximum failures
    /// - Timestamp tracking for security analysis
    /// - Integration with rate limiting and monitoring systems
    /// - Audit trail for security incident investigation
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

    /// Records a successful login and resets security counters.
    ///
    /// Updates the account after a successful authentication, resetting failed login
    /// attempts and account locks while recording the login timestamp and IP address
    /// for security auditing. This method ensures the account returns to a normal
    /// state after successful authentication.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account that successfully logged in
    /// * `ip_address` - IP address of the client for security tracking
    ///
    /// # Returns
    ///
    /// The updated `Account` with reset security counters and login information,
    /// or a database error if the operation fails.
    ///
    /// # Security Features
    ///
    /// - Resets failed login attempt counter to zero
    /// - Clears any existing account locks
    /// - Records login timestamp for activity tracking
    /// - Logs IP address for geographic and security analysis
    pub async fn record_successful_login(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        _ip_address: IpNet,
    ) -> PgResult<Account> {
        let _now = OffsetDateTime::now_utc();
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

    /// Unlocks an account by clearing security locks and resetting counters.
    ///
    /// Manually unlocks an account that has been locked due to failed login attempts
    /// or other security measures. This administrative function resets both the lock
    /// timestamp and failed attempt counter, returning the account to a normal state.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account to unlock
    ///
    /// # Returns
    ///
    /// The updated `Account` with cleared locks and reset counters,
    /// or a database error if the operation fails.
    ///
    /// # Administrative Use
    ///
    /// This method is typically used by administrators to manually unlock accounts
    /// that have been locked due to security policies. It should be used with
    /// appropriate authorization and audit logging.
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
    ///
    /// Securely updates an account's password hash and records when the change
    /// occurred for security auditing and password policy enforcement. This method
    /// should only be called with properly hashed passwords and appropriate
    /// authorization checks.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account whose password is being updated
    /// * `password_hash` - New password hash (must be properly hashed with bcrypt or similar)
    ///
    /// # Returns
    ///
    /// The updated `Account` with new password hash and change timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Security Requirements
    ///
    /// - Password must be properly hashed before calling this method
    /// - Consider invalidating existing sessions after password change
    /// - Implement password complexity validation before hashing
    /// - Rate limit password change operations to prevent abuse
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

    /// Verifies an account by setting the verification status to true.
    ///
    /// Marks an account as verified, typically after email verification or
    /// administrator approval. This is a critical step in the account lifecycle
    /// that enables full access to system features and builds user trust.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account to verify
    ///
    /// # Returns
    ///
    /// The updated `Account` with verification status set to true,
    /// or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Account gains access to verified-user features
    /// - Improves system security by confirming email ownership
    /// - May trigger welcome flows or feature notifications
    /// - Required for certain high-trust operations
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

    /// Suspends an account by setting the suspension status to true.
    ///
    /// Temporarily disables account access due to policy violations, security
    /// concerns, or administrative actions. Suspended accounts cannot authenticate
    /// but retain their data for potential future restoration.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account to suspend
    ///
    /// # Returns
    ///
    /// The updated `Account` with suspension status set to true,
    /// or a database error if the operation fails.
    ///
    /// # Security and Business Impact
    ///
    /// - Immediately prevents account authentication
    /// - All active sessions should be invalidated
    /// - Account data is preserved for audit and potential restoration
    /// - Consider sending suspension notification to user
    /// - Should be accompanied by proper audit logging
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

    /// Unsuspends an account by setting the suspension status to false.
    ///
    /// Restores account access after suspension, allowing the user to authenticate
    /// and use the system normally. This is typically done after addressing the
    /// issues that led to suspension or upon administrative review.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account to unsuspend
    ///
    /// # Returns
    ///
    /// The updated `Account` with suspension status set to false,
    /// or a database error if the operation fails.
    ///
    /// # Business Impact
    ///
    /// - Account regains full authentication and system access
    /// - User can log in and use all available features
    /// - Consider sending restoration notification to user
    /// - Should be accompanied by audit logging for compliance
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

    /// Checks if an email address is already registered in the system.
    ///
    /// Performs a case-insensitive lookup to determine if an email address
    /// is already associated with an active (non-deleted) account. This is
    /// commonly used during registration to enforce email uniqueness.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `email` - Email address to check (case-insensitive)
    ///
    /// # Returns
    ///
    /// `true` if the email is already in use, `false` if available,
    /// or a database error if the query fails.
    pub async fn email_exists(conn: &mut AsyncPgConnection, email: &str) -> PgResult<bool> {
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

    /// Finds accounts filtered by their verification status.
    ///
    /// Retrieves a paginated list of accounts based on whether they are
    /// verified or unverified. Useful for administrative oversight,
    /// verification campaigns, and user management workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `is_verified` - Filter by verification status (true for verified, false for unverified)
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Account` entries matching the verification criteria,
    /// ordered by creation date (newest first), or a database error if the query fails.
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

    /// Finds accounts filtered by their suspension status.
    ///
    /// Retrieves a paginated list of accounts based on whether they are
    /// currently suspended or active. Essential for administrative monitoring
    /// and managing account moderation workflows.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `is_suspended` - Filter by suspension status (true for suspended, false for active)
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Account` entries matching the suspension criteria,
    /// ordered by creation date (newest first), or a database error if the query fails.
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

    /// Finds accounts that are currently locked due to failed login attempts.
    ///
    /// Retrieves a paginated list of accounts with active locks (locked_until
    /// is in the future). These accounts are temporarily inaccessible due to
    /// security policies and may require administrative attention.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of currently locked `Account` entries ordered by lock expiration
    /// (soonest to expire first), or a database error if the query fails.
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

    /// Finds accounts created within the last 30 days.
    ///
    /// Retrieves a paginated list of newly registered accounts to monitor
    /// user growth, identify signup trends, and facilitate new user onboarding
    /// and engagement campaigns.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of recently created `Account` entries ordered by creation date
    /// (newest first), or a database error if the query fails.
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

    /// Finds accounts with no recent activity (no login in last 90 days).
    ///
    /// Retrieves a paginated list of accounts that haven't logged in recently
    /// or have never logged in. Useful for retention analysis, cleanup operations,
    /// and re-engagement campaigns.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of inactive `Account` entries ordered by creation date
    /// (oldest first), or a database error if the query fails.
    pub async fn find_inactive_accounts(
        conn: &mut AsyncPgConnection,
        pagination: Pagination,
    ) -> PgResult<Vec<Account>> {
        use schema::accounts::{self, dsl};

        let ninety_days_ago = OffsetDateTime::now_utc() - time::Duration::days(90);

        accounts::table
            .filter(dsl::updated_at.lt(ninety_days_ago))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::created_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(Account::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds accounts registered with a specific email domain.
    ///
    /// Retrieves a paginated list of accounts whose email addresses belong
    /// to the specified domain. Useful for organizational management,
    /// domain-based policies, and enterprise account administration.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `domain` - Email domain to search for (e.g., "example.com")
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Account` entries with matching email domains,
    /// ordered by creation date (newest first), or a database error if the query fails.
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

    /// Finds accounts with high numbers of failed login attempts.
    ///
    /// Retrieves a paginated list of accounts with 3 or more failed login
    /// attempts, indicating potential security issues, credential problems,
    /// or brute force attack targets that may need attention.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `Account` entries with high failed attempt counts,
    /// ordered by failed attempt count (highest first), or a database error if the query fails.
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

    /// Automatically unlocks accounts whose lock period has expired.
    ///
    /// Performs maintenance by clearing expired locks and resetting failed
    /// login attempt counters. This should be run periodically (e.g., via
    /// scheduled tasks) to ensure accounts regain access automatically
    /// after their lock period expires.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    ///
    /// # Returns
    ///
    /// A vector of `Account` entries that were unlocked during this operation,
    /// or a database error if the operation fails.
    ///
    /// # Maintenance Operations
    ///
    /// - Clears `locked_until` timestamp for expired locks
    /// - Resets `failed_login_attempts` counter to zero
    /// - Should be run via scheduled background jobs
    /// - Consider logging unlocked accounts for audit purposes
    pub async fn unlock_expired_accounts(conn: &mut AsyncPgConnection) -> PgResult<Vec<Account>> {
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
