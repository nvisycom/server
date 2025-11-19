//! Account action token repository for managing action token database operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{AccountActionToken, NewAccountActionToken, UpdateAccountActionToken};
use crate::types::ActionTokenType;
use crate::{PgError, PgResult, schema};

/// Repository for comprehensive account action token database operations.
///
/// Provides database operations for managing temporary action tokens used for
/// account-related actions like password resets, email verification, account
/// activation, and other time-sensitive operations. This repository handles
/// the full lifecycle of action tokens including creation, validation, usage
/// tracking, and cleanup operations.
///
/// Action tokens are security-sensitive components that require careful handling
/// to prevent abuse and ensure system security. All tokens have expiration times
/// and attempt limits to mitigate potential attacks.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountActionTokenRepository;

impl AccountActionTokenRepository {
    /// Creates a new account action token repository instance.
    ///
    /// Returns a new repository instance ready for database operations.
    /// Since the repository is stateless, this is equivalent to using
    /// `Default::default()` or accessing repository methods statically.
    ///
    /// # Returns
    ///
    /// A new `AccountActionTokenRepository` instance.
    pub fn new() -> Self {
        Self
    }

    // Token management methods

    /// Creates a new account action token for a specific action type.
    ///
    /// Generates a new time-limited token that can be used for account-related
    /// actions such as password resets, email verification, or account activation.
    /// The token is immediately persisted to the database with proper expiration
    /// and security constraints.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `new_token` - Complete token data including account ID, action type, and expiration
    ///
    /// # Returns
    ///
    /// The created `AccountActionToken` with database-generated ID and timestamp,
    /// or a database error if the operation fails.
    ///
    /// # Security Considerations
    ///
    /// - Token UUIDs should be cryptographically secure
    /// - Expiration times should be as short as practical for the use case
    /// - Consider rate limiting token creation to prevent abuse
    /// - Ensure proper cleanup of expired tokens
    pub async fn create_token(
        conn: &mut AsyncPgConnection,
        new_token: NewAccountActionToken,
    ) -> PgResult<AccountActionToken> {
        use schema::account_action_tokens;

        diesel::insert_into(account_action_tokens::table)
            .values(&new_token)
            .returning(AccountActionToken::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds a valid token by its UUID and action type.
    ///
    /// Retrieves an active (unused and unexpired) token that matches both the
    /// provided UUID and action type. This is the primary method for token
    /// validation during action processing. Only returns tokens that are
    /// still valid for use.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `token_uuid` - UUID of the token to find
    /// * `action` - Specific action type the token must match
    ///
    /// # Returns
    ///
    /// The matching `AccountActionToken` if found and valid, `None` if not found
    /// or invalid, or a database error if the query fails.
    ///
    /// # Validation Criteria
    ///
    /// - Token UUID must match exactly
    /// - Action type must match exactly
    /// - Token must not have been used (used_at is null)
    /// - Token must not be expired (expired_at > now)
    pub async fn find_token(
        conn: &mut AsyncPgConnection,
        token_uuid: Uuid,
        action: ActionTokenType,
    ) -> PgResult<Option<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        account_action_tokens::table
            .filter(dsl::action_token.eq(token_uuid))
            .filter(dsl::action_type.eq(action))
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.gt(OffsetDateTime::now_utc()))
            .select(AccountActionToken::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Finds the most recent valid token for an account and action type.
    ///
    /// Retrieves the newest active token for a specific account and action type.
    /// This is useful when you need to find any valid token for an account
    /// without knowing the specific token UUID. Returns the most recently
    /// issued token if multiple valid tokens exist.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to search tokens for
    /// * `action` - Specific action type to filter by
    ///
    /// # Returns
    ///
    /// The most recent valid `AccountActionToken` if found, `None` if no valid
    /// token exists, or a database error if the query fails. This enables
    /// checking if a user has pending password reset tokens before creating new ones.
    /// - Finding verification tokens during account activation
    /// - Administrative token management and oversight
    pub async fn find_account_token(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        action: ActionTokenType,
    ) -> PgResult<Option<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        account_action_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::action_type.eq(action))
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.gt(OffsetDateTime::now_utc()))
            .order(dsl::issued_at.desc())
            .select(AccountActionToken::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Finds any token by UUID regardless of action type or validity status.
    ///
    /// Retrieves a token using only its UUID, without filtering by action type,
    /// usage status, or expiration. This method is primarily used for
    /// administrative purposes, audit trails, or when you need to examine
    /// a token regardless of its current state.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `token_uuid` - UUID of the token to find
    ///
    /// # Returns
    ///
    /// A vector of `AccountActionToken` entries for the specified account and action type,
    /// or a database error if the query fails. This supports administrative token
    /// audit and investigation workflows.
    /// - Administrative token inspection
    /// - Debugging token-related issues
    /// - Security incident analysis
    pub async fn find_token_by_uuid(
        conn: &mut AsyncPgConnection,
        token_uuid: Uuid,
    ) -> PgResult<Option<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        account_action_tokens::table
            .filter(dsl::action_token.eq(token_uuid))
            .select(AccountActionToken::as_select())
            .first(conn)
            .await
            .optional()
            .map_err(PgError::from)
    }

    /// Updates a token's properties with new values.
    ///
    /// Applies partial updates to an existing token using the provided update
    /// structure. Only fields set to `Some(value)` will be modified, while
    /// `None` fields remain unchanged. Commonly used for updating attempt
    /// counts, expiration times, or usage status.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `token_uuid` - UUID of the token to update
    /// * `updates` - Partial update data containing only fields to modify
    ///
    /// # Returns
    ///
    /// The updated `AccountActionToken` with new values,
    /// or a database error if the operation fails.
    ///
    /// # Common Update Scenarios
    ///
    /// - Extending token expiration times
    /// - Updating attempt counts after failed validations
    /// - Marking tokens as used or invalidated
    /// - Administrative token modifications
    pub async fn update_token(
        conn: &mut AsyncPgConnection,
        token_uuid: Uuid,
        updates: UpdateAccountActionToken,
    ) -> PgResult<AccountActionToken> {
        use schema::account_action_tokens::{self, dsl};

        diesel::update(account_action_tokens::table.filter(dsl::action_token.eq(token_uuid)))
            .set(&updates)
            .returning(AccountActionToken::as_returning())
            .get_result(conn)
            .await
            .map_err(PgError::from)
    }

    /// Increments the attempt count for a token after failed validation.
    ///
    /// Increases the attempt counter by one to track failed token usage attempts.
    /// This is crucial for security monitoring and preventing brute force attacks
    /// on tokens. Should be called whenever a token is presented but fails
    /// validation for any reason.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `token_uuid` - UUID of the token to update
    /// * `account_id` - UUID of the account (for additional security validation)
    ///
    /// # Returns
    ///
    /// The updated `AccountActionToken` with incremented attempt count,
    /// or a database error if the operation fails.
    ///
    /// # Security Benefits
    ///
    /// - Tracks suspicious activity patterns
    /// - Enables automatic token invalidation after too many attempts
    /// - Provides audit trail for security investigations
    /// - Helps identify potential brute force attacks
    pub async fn increment_token_attempts(
        conn: &mut AsyncPgConnection,
        token_uuid: Uuid,
        account_id: Uuid,
    ) -> PgResult<AccountActionToken> {
        use schema::account_action_tokens::{self, dsl};

        diesel::update(
            account_action_tokens::table
                .filter(dsl::action_token.eq(token_uuid))
                .filter(dsl::account_id.eq(account_id)),
        )
        .set(dsl::attempt_count.eq(dsl::attempt_count + 1))
        .returning(AccountActionToken::as_returning())
        .get_result(conn)
        .await
        .map_err(PgError::from)
    }

    /// Marks a token as used after successful action completion.
    ///
    /// Sets the token's usage timestamp to prevent reuse and maintain security.
    /// This should be called immediately after successfully processing the
    /// action the token was created for. Once marked as used, the token
    /// becomes invalid for future operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `token_uuid` - UUID of the token to mark as used
    /// * `account_id` - UUID of the account (for additional security validation)
    ///
    /// # Returns
    ///
    /// The updated `AccountActionToken` with usage timestamp set,
    /// or a database error if the operation fails.
    ///
    /// # Security and Operational Impact
    ///
    /// - Prevents token reuse attacks
    /// - Provides audit trail of successful actions
    /// - Enables proper token lifecycle management
    /// - Critical for maintaining action authenticity
    pub async fn use_token(
        conn: &mut AsyncPgConnection,
        token_uuid: Uuid,
        account_id: Uuid,
    ) -> PgResult<AccountActionToken> {
        use schema::account_action_tokens::{self, dsl};

        diesel::update(
            account_action_tokens::table
                .filter(dsl::action_token.eq(token_uuid))
                .filter(dsl::account_id.eq(account_id)),
        )
        .set(dsl::used_at.eq(Some(OffsetDateTime::now_utc())))
        .returning(AccountActionToken::as_returning())
        .get_result(conn)
        .await
        .map_err(PgError::from)
    }

    /// Invalidates a token by marking it as used without action completion.
    ///
    /// Marks a token as used to prevent further use, typically for security
    /// reasons or administrative actions. Unlike `use_token`, this method
    /// doesn't require account validation and can invalidate tokens even
    /// when the associated action wasn't completed.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `token_uuid` - UUID of the token to invalidate
    ///
    /// # Returns
    ///
    /// The number of tokens that were invalidated,
    /// or a database error if the operation fails. This enables
    /// emergency token revocation for security incidents.
    /// - Administrative security actions
    /// - Bulk token invalidation during security incidents
    /// - Cleanup of potentially compromised tokens
    pub async fn invalidate_token(
        conn: &mut AsyncPgConnection,
        token_uuid: Uuid,
    ) -> PgResult<bool> {
        use schema::account_action_tokens::{self, dsl};

        let rows_affected =
            diesel::update(account_action_tokens::table.filter(dsl::action_token.eq(token_uuid)))
                .set(dsl::used_at.eq(Some(OffsetDateTime::now_utc())))
                .execute(conn)
                .await
                .map_err(PgError::from)?;

        Ok(rows_affected > 0)
    }

    /// Lists tokens for a specific account with optional filtering.
    ///
    /// Retrieves a paginated list of tokens associated with an account.
    /// Can optionally include or exclude used tokens based on the use case.
    /// Results are ordered by issue date with most recent tokens first.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `account_id` - UUID of the account to list tokens for
    /// * `include_used` - Whether to include tokens that have been used
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `AccountActionToken` entries for the account,
    /// ordered by issue date (newest first), or a database error if the query fails.
    /// This supports administrative account token audit and review processes.
    /// - Security monitoring and investigation
    /// - User support and troubleshooting
    /// - Token lifecycle management
    pub async fn list_account_tokens(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        include_used: bool,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        let mut query = account_action_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .order(dsl::issued_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountActionToken::as_select())
            .into_boxed();

        if !include_used {
            query = query.filter(dsl::used_at.is_null());
        }

        query.load(conn).await.map_err(PgError::from)
    }

    /// Lists tokens filtered by action type with comprehensive filtering options.
    ///
    /// Retrieves a paginated list of tokens for a specific action type across
    /// all accounts. Provides flexible filtering options for used and expired
    /// tokens to support various administrative and analytical use cases.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `action` - Action type to filter tokens by
    /// * `include_used` - Whether to include tokens that have been used
    /// * `include_expired` - Whether to include expired tokens
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `AccountActionToken` entries matching the criteria,
    /// A vector of `AccountActionToken` entries for the specified action type,
    /// ordered by issue date (newest first), or a database error if the query fails.
    /// This enables system-wide token usage analysis and administrative oversight.
    /// - Action type performance monitoring
    /// - Security pattern detection
    /// - Bulk token management operations
    pub async fn list_tokens_by_action(
        conn: &mut AsyncPgConnection,
        action: ActionTokenType,
        include_used: bool,
        include_expired: bool,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        let mut query = account_action_tokens::table
            .filter(dsl::action_type.eq(action))
            .order(dsl::issued_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountActionToken::as_select())
            .into_boxed();

        if !include_used {
            query = query.filter(dsl::used_at.is_null());
        }

        if !include_expired {
            query = query.filter(dsl::expired_at.gt(OffsetDateTime::now_utc()));
        }

        query.load(conn).await.map_err(PgError::from)
    }

    /// Retrieves comprehensive token statistics for a specific account.
    ///
    /// Generates statistical information about all tokens associated with
    /// an account, including counts of active, used, expired, and total tokens.
    /// This data is essential for account monitoring, security analysis,
    /// and user support operations.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the queries
    /// * `account_id` - UUID of the account to generate statistics for
    ///
    /// # Returns
    ///
    /// An `AccountActionTokenStats` struct containing comprehensive token metrics,
    /// or a database error if any of the queries fail.
    ///
    /// # Performance Considerations
    ///
    /// - Executes multiple database queries for different metrics
    /// - Results can be cached for dashboard performance
    /// - Database indexes optimize count operations
    pub async fn get_account_token_stats(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<AccountActionTokenStats> {
        use schema::account_action_tokens::{self, dsl};

        let now = OffsetDateTime::now_utc();

        // Count active (unused, unexpired) tokens
        let active_tokens = account_action_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.gt(now))
            .count()
            .get_result::<i64>(conn)
            .await
            .map_err(PgError::from)?;

        // Count used tokens
        let used_tokens = account_action_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::used_at.is_not_null())
            .count()
            .get_result::<i64>(conn)
            .await
            .map_err(PgError::from)?;

        // Count expired tokens
        let expired_tokens = account_action_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.le(now))
            .count()
            .get_result::<i64>(conn)
            .await
            .map_err(PgError::from)?;

        // Count total tokens
        let total_tokens = account_action_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .count()
            .get_result::<i64>(conn)
            .await
            .map_err(PgError::from)?;

        Ok(AccountActionTokenStats {
            active_tokens,
            used_tokens,
            expired_tokens,
            total_tokens,
        })
    }

    /// Finds tokens that are approaching their expiration time.
    ///
    /// Identifies tokens that will expire within the specified time window.
    /// This is useful for proactive notifications, cleanup scheduling, and
    /// monitoring token usage patterns. Only returns unused tokens since
    /// used tokens don't need expiration warnings.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `expires_within` - Time duration window for finding expiring tokens
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `AccountActionToken` entries expiring within the time window,
    /// A vector of `AccountActionToken` entries expiring soon,
    /// ordered by expiration time (soonest first), or a database error if the query fails.
    /// This enables proactive user notifications about expiring tokens.
    /// - Automated cleanup scheduling
    /// - Token usage pattern analysis
    /// - System maintenance planning
    pub async fn find_expiring_tokens(
        conn: &mut AsyncPgConnection,
        expires_within: time::Duration,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        let expiry_threshold = OffsetDateTime::now_utc() + expires_within;

        account_action_tokens::table
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.le(expiry_threshold))
            .filter(dsl::expired_at.gt(OffsetDateTime::now_utc()))
            .order(dsl::expired_at.asc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountActionToken::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Finds tokens with high attempt counts indicating potential security issues.
    ///
    /// Identifies tokens that have accumulated many failed validation attempts,
    /// which may indicate brute force attacks, user confusion, or system issues.
    /// Only returns unused tokens since used tokens are no longer a concern.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the query
    /// * `min_attempts` - Minimum attempt count threshold for inclusion
    /// * `pagination` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    ///
    /// A vector of `AccountActionToken` entries with high failure rates,
    /// ordered by attempt count (highest first), or a database error if the query fails.
    /// This supports security incident detection and response workflows.
    /// - Identifying potential brute force attacks
    /// - User support for confused or struggling users
    /// - System abuse pattern analysis
    /// - Automated security alerting
    pub async fn find_high_attempt_tokens(
        conn: &mut AsyncPgConnection,
        min_attempts: i32,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        account_action_tokens::table
            .filter(dsl::attempt_count.ge(min_attempts))
            .filter(dsl::used_at.is_null())
            .order(dsl::attempt_count.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountActionToken::as_select())
            .load(conn)
            .await
            .map_err(PgError::from)
    }

    /// Invalidates all unused tokens for an account with optional action filtering.
    ///
    /// Marks all unused tokens for a specific account as used to prevent further
    /// access. Can optionally filter by action type to invalidate only specific
    /// types of tokens. This is commonly used during security incidents,
    /// password changes, or account state transitions.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - UUID of the account whose tokens should be invalidated
    /// * `action` - Optional action type filter (None invalidates all types)
    ///
    /// # Returns
    ///
    /// The number of tokens that were invalidated,
    /// or a database error if the operation fails. This supports password
    /// change security by invalidating password reset tokens and other scenarios
    /// requiring bulk token revocation.
    /// - Account suspension procedures
    /// - Security incident response
    /// - User-requested token revocation
    /// - Administrative security actions
    pub async fn invalidate_account_tokens(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        action: Option<ActionTokenType>,
    ) -> PgResult<i64> {
        use schema::account_action_tokens::{self, dsl};

        let mut query = diesel::update(
            account_action_tokens::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::used_at.is_null()),
        )
        .into_boxed();

        if let Some(action_type) = action {
            query = query.filter(dsl::action_type.eq(action_type));
        }

        query
            .set(dsl::used_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }

    // Cleanup and maintenance methods

    /// Performs cleanup of expired and used tokens with optional account filtering.
    ///
    /// Permanently deletes tokens that are either expired or have been used,
    /// helping maintain database performance and reducing storage requirements.
    /// Can operate on all accounts or be limited to a specific account.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `account_id` - Optional account UUID to limit cleanup scope (None for all accounts)
    ///
    /// # Returns
    ///
    /// The number of tokens that were deleted,
    /// or a database error if the operation fails.
    ///
    /// # Maintenance Benefits
    ///
    /// - Improves database performance by reducing table size
    /// - Frees storage space from obsolete tokens
    /// - Maintains system hygiene and security
    /// - Should be run regularly via scheduled jobs
    ///
    /// # Caution
    ///
    /// This operation permanently deletes data and cannot be undone.
    /// Consider audit requirements before implementing automated cleanup.
    pub async fn cleanup_expired_tokens(
        conn: &mut AsyncPgConnection,
        account_id: Option<Uuid>,
    ) -> PgResult<i64> {
        use schema::account_action_tokens::{self, dsl};

        let mut query = diesel::delete(
            account_action_tokens::table.filter(
                dsl::expired_at
                    .lt(OffsetDateTime::now_utc())
                    .or(dsl::used_at.is_not_null()),
            ),
        )
        .into_boxed();

        if let Some(acc_id) = account_id {
            query = query.filter(dsl::account_id.eq(acc_id));
        }

        query
            .execute(conn)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }

    /// Permanently deletes old used and expired tokens beyond retention period.
    ///
    /// Performs aggressive cleanup of tokens that are significantly old,
    /// regardless of their usage status. This is typically used for compliance
    /// with data retention policies and long-term database maintenance.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `older_than_days` - Age threshold in days for token deletion
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
    /// - Should align with audit and legal requirements
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
        use schema::account_action_tokens::{self, dsl};

        let cutoff_date = OffsetDateTime::now_utc() - time::Duration::days(older_than_days as i64);

        diesel::delete(
            account_action_tokens::table.filter(
                dsl::used_at
                    .is_not_null()
                    .and(dsl::used_at.lt(cutoff_date))
                    .or(dsl::expired_at.lt(cutoff_date)),
            ),
        )
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }

    /// Invalidates tokens with excessive failed attempts for security purposes.
    ///
    /// Marks tokens with high attempt counts as used to prevent further
    /// brute force attempts. This is a security measure that automatically
    /// disables tokens that show signs of being under attack or causing
    /// user confusion with repeated failed attempts.
    ///
    /// # Arguments
    ///
    /// * `conn` - Active database connection for the operation
    /// * `max_attempts` - Maximum allowed attempts before token invalidation
    ///
    /// # Returns
    ///
    /// The number of tokens that were invalidated due to high attempt counts,
    /// or a database error if the operation fails.
    ///
    /// # Security Benefits
    ///
    /// - Prevents brute force attacks on tokens
    /// - Reduces system load from repeated failed attempts
    /// - Improves overall security posture
    /// - Can be automated for continuous protection
    ///
    /// # Operational Considerations
    ///
    /// - Should be balanced with user experience
    /// - Consider user notification before invalidation
    /// - May require user support for legitimate high-attempt scenarios
    pub async fn cleanup_high_attempt_tokens(
        conn: &mut AsyncPgConnection,
        max_attempts: i32,
    ) -> PgResult<i64> {
        use schema::account_action_tokens::{self, dsl};

        diesel::update(account_action_tokens::table.filter(dsl::attempt_count.ge(max_attempts)))
            .set(dsl::used_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }
}

/// Comprehensive statistical information about account action tokens.
///
/// Provides aggregated counts and metrics for different token states and
/// characteristics for a specific account. This data structure is used for
/// account monitoring, security analysis, and administrative dashboards.
///
/// All statistics represent the current state of tokens and can be used
/// to calculate derived metrics through the implemented methods.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountActionTokenStats {
    /// Number of active (unused, unexpired) tokens
    pub active_tokens: i64,
    /// Number of used tokens
    pub used_tokens: i64,
    /// Number of expired tokens
    pub expired_tokens: i64,
    /// Total number of tokens for the account
    pub total_tokens: i64,
}

impl AccountActionTokenStats {
    /// Calculates the percentage of tokens that have been successfully used.
    ///
    /// Returns the ratio of used tokens to total tokens as a percentage.
    /// A higher usage rate indicates effective token utilization and successful
    /// completion of intended actions.
    ///
    /// # Returns
    ///
    /// A percentage value between 0.0 and 100.0. Returns 0.0 if there are
    /// no tokens for the account. This metric is essential for measuring
    /// token effectiveness and system usability.
    /// - Identifying user engagement with email workflows
    /// - Detecting potential token delivery issues
    /// - Performance metrics for token-based processes
    pub fn usage_rate(&self) -> f64 {
        if self.total_tokens == 0 {
            0.0
        } else {
            (self.used_tokens as f64 / self.total_tokens as f64) * 100.0
        }
    }

    /// Calculates the percentage of tokens that expired without being used.
    ///
    /// Returns the ratio of expired tokens to total tokens as a percentage.
    /// A high expiration rate may indicate issues with token delivery,
    /// user engagement, or token validity periods.
    ///
    /// # Returns
    ///
    /// A percentage value between 0.0 and 100.0. Returns 0.0 if there are
    /// no tokens for the account. This metric helps identify token delivery
    /// problems and system reliability issues.
    /// - Optimizing token validity periods
    /// - Measuring user response times
    /// - Detecting engagement issues
    pub fn expiration_rate(&self) -> f64 {
        if self.total_tokens == 0 {
            0.0
        } else {
            (self.expired_tokens as f64 / self.total_tokens as f64) * 100.0
        }
    }

    /// Checks if the account currently has any active (unused, unexpired) tokens.
    ///
    /// Returns true if there are tokens that can still be used for their
    /// intended actions. Useful for preventing duplicate token creation
    /// and checking account token status.
    ///
    /// # Returns
    ///
    /// `true` if there are active tokens, `false` otherwise. This helps
    /// prevent duplicate password reset emails and other redundant operations.
    /// - Checking if verification is pending
    /// - Conditional UI display logic
    /// - Rate limiting token generation
    pub fn has_active_tokens(&self) -> bool {
        self.active_tokens > 0
    }

    /// Checks if the account has expired tokens that could be cleaned up.
    ///
    /// Returns true if there are tokens that have expired without being used.
    /// These tokens are candidates for cleanup operations to maintain
    /// database hygiene and performance.
    ///
    /// # Returns
    ///
    /// `true` if there are expired tokens, `false` otherwise. This enables
    /// triggering cleanup operations and maintenance workflows.
    /// - Database maintenance scheduling
    /// - Storage optimization decisions
    /// - Performance monitoring
    pub fn has_expired_tokens(&self) -> bool {
        self.expired_tokens > 0
    }
}
