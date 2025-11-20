//! Account API token repository for managing token database operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive account API token database operations.
///
/// Provides database operations for managing long-lived API tokens used for
/// programmatic access to the system. These tokens enable applications and
/// services to authenticate and access resources on behalf of user accounts.
/// This repository handles the complete lifecycle of API tokens including
/// creation, refresh, validation, and cleanup operations.
///
/// API tokens are security-critical components that require careful handling
/// to prevent unauthorized access. All tokens have expiration times and
/// usage tracking to maintain security and enable proper audit trails.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountApiTokenRepository;

impl AccountApiTokenRepository {
    /// Creates a new account API token repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `AccountApiTokenRepository` instance.
    pub fn new() -> Self {
        Self
    }

    // Token management methods

    /// Creates a new account API token for programmatic access.
    ///
    /// Generates a new long-lived API token that applications can use for
    /// authentication and API access. The token includes both access and
    /// refresh sequences for secure token rotation and extended validity.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `new_token` - Complete token data including account ID, expiration, and metadata
    ///
    /// # Returns
    ///
    /// The created `AccountApiToken` with database-generated ID and timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Security Considerations
    ///
    /// - Token sequences should be cryptographically secure UUIDs
    /// - Expiration times should balance usability with security
    /// - Consider rate limiting token creation per account
    /// - Implement proper token rotation policies
    pub async fn create_token(
        conn: &mut AsyncPgConnection,
        new_token: NewAccountApiToken,
    ) -> PgResult<AccountApiToken> {
        use schema::account_api_tokens;

        diesel::insert_into(account_api_tokens::table)
            .values(&new_token)
            .returning(AccountApiToken::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds an active token by its access token sequence.
    ///
    /// Retrieves a token using its access sequence UUID for authentication
    /// purposes. Only returns non-deleted tokens that can be used for API
    /// access. This is the primary method for token-based authentication.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `access_token` - Access sequence UUID to search for
    ///
    /// # Returns
    ///
    /// The matching `AccountApiToken` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
    ///
    /// # Authentication Flow
    ///
    /// - Used during API request authentication
    /// - Should be followed by expiration time validation
    /// - Consider updating last_used_at timestamp after successful use
    /// - Implement rate limiting to prevent token enumeration attacks
    pub async fn find_token_by_access_token(
        conn: &mut AsyncPgConnection,
        access_token: Uuid,
    ) -> PgResult<Option<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::access_seq.eq(access_token))
            .filter(dsl::deleted_at.is_null())
            .select(AccountApiToken::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Finds an active token by its refresh token sequence.
    ///
    /// Retrieves a token using its refresh sequence UUID for token refresh
    /// operations. Only returns non-deleted tokens. The refresh token is
    /// used to generate new access/refresh token pairs without requiring
    /// user re-authentication.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `refresh_token` - Refresh sequence UUID to search for
    ///
    /// # Returns
    ///
    /// The matching `AccountApiToken` if found and not deleted, `None` if not found,
    /// or a database error if the query fails.
    ///
    /// # Token Refresh Flow
    ///
    /// - Used during token refresh operations
    /// - Should validate token expiration before refresh
    /// - Generates new access and refresh sequences
    /// - Critical for maintaining long-term API access
    pub async fn find_token_by_refresh_token(
        conn: &mut AsyncPgConnection,
        refresh_token: Uuid,
    ) -> PgResult<Option<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::refresh_seq.eq(refresh_token))
            .filter(dsl::deleted_at.is_null())
            .select(AccountApiToken::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Updates a token's properties with new values.
    ///
    /// Applies partial updates to an existing token using the provided update
    /// structure. Only fields set to `Some(value)` will be modified, while
    /// `None` fields remain unchanged. Commonly used for updating usage
    /// timestamps, expiration times, or metadata.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `access_token` - Access sequence UUID of the token to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `AccountApiToken` with new values,
    /// or a database error if the operation fails.
    ///
    /// # Common Update Scenarios
    ///
    /// - Updating last usage timestamps
    /// - Extending token expiration times
    /// - Modifying token metadata
    /// - Administrative token adjustments
    pub async fn update_token(
        conn: &mut AsyncPgConnection,
        access_token: Uuid,
        updates: UpdateAccountApiToken,
    ) -> PgResult<AccountApiToken> {
        use schema::account_api_tokens::{self, dsl};

        diesel::update(account_api_tokens::table.filter(dsl::access_seq.eq(access_token)))
            .set(&updates)
            .returning(AccountApiToken::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Updates the token's last used timestamp to track usage patterns.
    ///
    /// Records the current time as the last usage timestamp for the token.
    /// This is important for security monitoring, usage analytics, and
    /// identifying inactive tokens that can be cleaned up.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `access_token` - Access sequence UUID of the token to update
    ///
    /// # Returns
    ///
    /// The updated `AccountApiToken` with current timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Usage Tracking Benefits
    ///
    /// - Enables identification of inactive tokens
    /// - Supports security monitoring and alerting
    /// - Provides data for usage analytics
    /// - Helps with token lifecycle management
    pub async fn touch_token(
        conn: &mut AsyncPgConnection,
        access_token: Uuid,
    ) -> PgResult<AccountApiToken> {
        Self::update_token(
            conn,
            access_token,
            UpdateAccountApiToken {
                last_used_at: Some(OffsetDateTime::now_utc()),
                ..Default::default()
            },
        )
        .await
    }

    /// Extends a token's expiration time by the specified duration.
    ///
    /// Adds the given time duration to the current time to create a new
    /// expiration timestamp, effectively extending the token's validity period.
    /// Also updates the last used timestamp to reflect the extension activity.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `access_token` - Access sequence UUID of the token to extend
    /// * `extension` - Time duration to add to the current time
    ///
    /// # Returns
    ///
    /// The updated `AccountApiToken` with new expiration time,
    /// or a database error if the operation fails.
    pub async fn extend_token(
        conn: &mut AsyncPgConnection,
        access_token: Uuid,
        extension: time::Duration,
    ) -> PgResult<AccountApiToken> {
        let new_expiry = OffsetDateTime::now_utc() + extension;
        Self::update_token(
            conn,
            access_token,
            UpdateAccountApiToken {
                expired_at: Some(new_expiry),
                last_used_at: Some(OffsetDateTime::now_utc()),
                ..Default::default()
            },
        )
        .await
    }

    /// Refreshes a token by generating new access and refresh sequences.
    ///
    /// Creates new cryptographically secure UUIDs for both access and refresh
    /// sequences, extends the expiration time, and updates the last used timestamp.
    /// This operation maintains API access without requiring user re-authentication.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `refresh_token` - Current refresh sequence UUID to be replaced
    ///
    /// # Returns
    ///
    /// The updated `AccountApiToken` with new sequences and expiration,
    /// or a database error if the operation fails.
    ///
    /// # Security Benefits
    ///
    /// - Prevents token replay attacks through rotation
    /// - Limits exposure window of compromised tokens
    /// - Maintains audit trail of token usage
    /// - Enables long-term API access without stored credentials
    ///
    /// # Default Behavior
    ///
    /// - Sets expiration to 7 days from current time
    /// - Generates new random UUIDs for both sequences
    /// - Updates last used timestamp to current time
    pub async fn refresh_token(
        conn: &mut AsyncPgConnection,
        refresh_token: Uuid,
    ) -> PgResult<AccountApiToken> {
        use schema::account_api_tokens::{self, dsl};

        let new_access_seq = Uuid::new_v4();
        let new_refresh_seq = Uuid::new_v4();
        let new_expiry = OffsetDateTime::now_utc() + time::Duration::days(7);

        diesel::update(account_api_tokens::table.filter(dsl::refresh_seq.eq(refresh_token)))
            .set((
                dsl::access_seq.eq(new_access_seq),
                dsl::refresh_seq.eq(new_refresh_seq),
                dsl::expired_at.eq(new_expiry),
                dsl::last_used_at.eq(Some(OffsetDateTime::now_utc())),
            ))
            .returning(AccountApiToken::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Soft deletes a token to effectively log out the associated session.
    ///
    /// Marks the token as deleted by setting the deletion timestamp, which
    /// immediately prevents the token from being used for authentication.
    /// The token record is preserved for audit purposes and potential
    /// recovery scenarios.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `access_token` - Access sequence UUID of the token to delete
    ///
    /// # Returns
    ///
    /// `true` if a token was successfully deleted, `false` if no token was found,
    /// or a database error if the operation fails.
    ///
    /// # Logout and Security Benefits
    ///
    /// - Immediately invalidates API access
    /// - Prevents unauthorized use of potentially compromised tokens
    /// - Maintains audit trail of logout events
    /// - Supports user-initiated security actions
    pub async fn delete_token(conn: &mut AsyncPgConnection, access_token: Uuid) -> PgResult<bool> {
        use schema::account_api_tokens::{self, dsl};

        let rows_affected =
            diesel::update(account_api_tokens::table.filter(dsl::access_seq.eq(access_token)))
                .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
                .execute(conn)
                .await
                .map_err(PgError::from)?;

        Ok(rows_affected > 0)
    }

    /// Soft deletes all active tokens for an account to log out all sessions.
    ///
    /// Marks all non-deleted tokens for the specified account as deleted,
    /// effectively logging out all API sessions and applications. This is
    /// commonly used for security incidents, password changes, or user-requested
    /// global logout operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account whose tokens should be deleted
    ///
    /// # Returns
    ///
    /// The number of tokens that were deleted,
    /// or a database error if the operation fails.
    ///
    /// # Security Use Cases
    ///
    /// - Emergency security response to compromised accounts
    /// - Password change security procedures
    /// - User-requested global logout from all devices
    /// - Account suspension or termination processes
    /// - Compliance with security policies
    pub async fn delete_all_tokens_for_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<i64> {
        use schema::account_api_tokens::{self, dsl};

        diesel::update(
            account_api_tokens::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }

    /// Lists currently active (non-deleted, unexpired) tokens for an account.
    ///
    /// Retrieves a paginated list of tokens that are currently valid for API
    /// access. Only includes tokens that haven't been deleted and haven't
    /// expired, providing a view of the account's current API access capabilities.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to list tokens for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of active `AccountApiToken` entries for the account,
    /// ordered by issue date (newest first), or a database error if the query fails.
    pub async fn list_account_tokens(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::expired_at.gt(OffsetDateTime::now_utc()))
            .order(dsl::issued_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountApiToken::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Lists all non-deleted tokens for an account including expired tokens.
    ///
    /// Retrieves a comprehensive paginated list of all tokens for an account,
    /// including those that have expired but haven't been deleted. This provides
    /// a complete view of the account's API token history for audit and
    /// administrative purposes.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to list tokens for
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of all non-deleted `AccountApiToken` entries for the account,
    /// ordered by issue date (newest first), or a database error if the query fails.
    ///
    /// # Administrative Use Cases
    ///
    /// - Complete audit trail review
    /// - Token usage pattern analysis
    /// - Security incident investigation
    /// - Compliance reporting and documentation
    pub async fn list_all_account_tokens(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::issued_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountApiToken::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds tokens for an account that are approaching their expiration time.
    ///
    /// Identifies active tokens that will expire within the specified time window.
    /// This is useful for proactive notifications, automatic refresh scheduling,
    /// and preventing unexpected API access interruptions.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to check for expiring tokens
    /// * `expires_within` - Time duration window for finding expiring tokens
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `AccountApiToken` entries expiring within the time window,
    /// ordered by expiration time (soonest first), or a database error if the query fails.
    ///
    /// - API client maintenance planning
    pub async fn find_expiring_tokens(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        expires_within: time::Duration,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        let expiry_threshold = OffsetDateTime::now_utc() + expires_within;

        account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::expired_at.le(expiry_threshold))
            .filter(dsl::expired_at.gt(OffsetDateTime::now_utc()))
            .order(dsl::expired_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountApiToken::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds the most recently used token for an account.
    ///
    /// Retrieves the token with the most recent last_used_at timestamp,
    /// providing insight into the account's most active API access pattern.
    /// If no tokens have been used, returns the most recently issued token.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to find the latest token for
    ///
    /// # Returns
    ///
    /// The most recently used `AccountApiToken` if found, `None` if no tokens exist,
    /// or a database error if the query fails.
    ///
    /// # Analysis Use Cases
    ///
    /// - Determining primary API access patterns
    /// - Identifying the most active token for renewal
    /// - User support and troubleshooting
    /// - Security monitoring for account activity
    pub async fn find_latest_token(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<Option<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::last_used_at.desc().nulls_last())
            .select(AccountApiToken::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    // Cleanup and maintenance methods

    /// Performs system-wide cleanup of expired tokens by soft-deleting them.
    ///
    /// Marks all expired tokens across all accounts as deleted to maintain
    /// system performance and security hygiene. This operation should be
    /// run regularly as part of system maintenance to prevent accumulation
    /// of unusable tokens.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    ///
    /// # Returns
    ///
    /// The number of tokens that were marked as deleted,
    /// or a database error if the operation fails.
    ///
    /// # Maintenance Benefits
    ///
    /// - Improves query performance by reducing active token count
    /// - Enhances security by removing expired access vectors
    /// - Maintains system hygiene and cleanliness
    /// - Should be automated via scheduled maintenance jobs
    ///
    /// # Scheduling Recommendation
    ///
    /// Run this operation daily or weekly depending on token volume
    /// and expiration patterns to maintain optimal performance.
    pub async fn cleanup_expired_tokens(conn: &mut AsyncPgConnection) -> PgResult<i64> {
        use schema::account_api_tokens::{self, dsl};

        diesel::update(
            account_api_tokens::table
                .filter(dsl::expired_at.lt(OffsetDateTime::now_utc()))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }

    /// Permanently deletes old soft-deleted tokens beyond retention period.
    ///
    /// Performs aggressive cleanup by permanently removing tokens that have
    /// been soft-deleted for longer than the specified retention period.
    /// This is used for compliance with data retention policies and
    /// long-term database maintenance.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `older_than_days` - Age threshold in days for permanent deletion
    ///
    /// # Returns
    ///
    /// The number of tokens that were permanently deleted,
    /// or a database error if the operation fails.
    ///
    /// # Data Retention and Compliance
    ///
    /// - Supports regulatory compliance requirements
    /// - Implements data retention policies
    /// - Reduces long-term storage costs
    /// - Must align with audit and legal requirements
    ///
    /// # Critical Warning
    ///
    /// This operation permanently and irreversibly deletes data.
    /// Ensure compliance with legal and audit requirements before
    /// implementing automated purging processes.
    pub async fn purge_old_tokens(
        conn: &mut AsyncPgConnection,
        older_than_days: u32,
    ) -> PgResult<i64> {
        use schema::account_api_tokens::{self, dsl};

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(older_than_days as i64);

        diesel::delete(account_api_tokens::table.filter(dsl::deleted_at.lt(cutoff_date)))
            .execute(conn)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }

    /// Revokes tokens older than specified duration for security purposes.
    ///
    /// Soft-deletes tokens that have been issued longer ago than the specified
    /// duration, regardless of their expiration time. This is a security measure
    /// to prevent very old tokens from remaining active and potentially
    /// compromised over time.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `older_than` - Maximum age duration for tokens to remain active
    ///
    /// # Returns
    ///
    /// The number of tokens that were revoked (soft-deleted),
    /// or a database error if the operation fails.
    ///
    /// # Security Benefits
    ///
    /// - Enforces maximum token lifetime policies
    /// - Reduces exposure window for potentially compromised tokens
    /// - Encourages regular token refresh cycles
    /// - Supports compliance with security frameworks
    ///
    /// # Policy Implementation
    ///
    /// Can be used to implement organizational policies requiring
    /// periodic token rotation regardless of expiration times.
    pub async fn revoke_old_tokens(
        conn: &mut AsyncPgConnection,
        older_than: time::Duration,
    ) -> PgResult<i64> {
        use schema::account_api_tokens::{self, dsl};

        let cutoff_date = OffsetDateTime::now_utc() - older_than;

        diesel::update(
            account_api_tokens::table
                .filter(dsl::issued_at.lt(cutoff_date))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(Some(OffsetDateTime::now_utc())))
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }
}
