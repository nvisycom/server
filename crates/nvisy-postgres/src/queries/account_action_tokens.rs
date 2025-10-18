//! Account action token repository for managing action token database operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::models::{AccountActionToken, NewAccountActionToken, UpdateAccountActionToken};
use crate::types::ActionTokenType;
use crate::{PgError, PgResult, schema};

/// Repository for account action token-related database operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountActionTokenRepository;

impl AccountActionTokenRepository {
    /// Creates a new account action token repository instance.
    pub fn new() -> Self {
        Self
    }

    // Token management methods

    /// Creates a new account action token.
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

    /// Finds a token by its UUID and action type.
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

    /// Finds a valid token for an account and action type.
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

    /// Finds any token by UUID (regardless of action type or status).
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

    /// Updates a token's properties.
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

    /// Increments the attempt count for a token.
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

    /// Marks a token as used.
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

    /// Invalidates a token by marking it as used.
    pub async fn invalidate_token(
        conn: &mut AsyncPgConnection,
        token_uuid: Uuid,
    ) -> PgResult<bool> {
        use schema::account_action_tokens::{self, dsl};

        let rows_affected = diesel::update(account_action_tokens::table.filter(dsl::action_token.eq(token_uuid)))
            .set(dsl::used_at.eq(Some(OffsetDateTime::now_utc())))
            .execute(conn)
            .await
            .map_err(PgError::from)?;

        Ok(rows_affected > 0)
    }

    /// Lists tokens for an account.
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

    /// Lists tokens by action type.
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

    /// Gets token statistics for an account.
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

    /// Finds tokens that are about to expire.
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

    /// Finds tokens with high attempt counts (potential security concern).
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

    /// Invalidates all unused tokens for an account and action type.
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

    /// Deletes expired tokens for an account.
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

    /// Hard deletes old used tokens.
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

    /// Cleanup tokens with too many failed attempts.
    pub async fn cleanup_high_attempt_tokens(
        conn: &mut AsyncPgConnection,
        max_attempts: i32,
    ) -> PgResult<i64> {
        use schema::account_action_tokens::{self, dsl};

        diesel::update(
            account_action_tokens::table.filter(dsl::attempt_count.ge(max_attempts))
        )
        .set(dsl::used_at.eq(Some(OffsetDateTime::now_utc())))
        .execute(conn)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }
}

/// Statistics for account action tokens.
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
    /// Returns the usage rate as a percentage (0-100).
    pub fn usage_rate(&self) -> f64 {
        if self.total_tokens == 0 {
            0.0
        } else {
            (self.used_tokens as f64 / self.total_tokens as f64) * 100.0
        }
    }

    /// Returns the expiration rate as a percentage (0-100).
    pub fn expiration_rate(&self) -> f64 {
        if self.total_tokens == 0 {
            0.0
        } else {
            (self.expired_tokens as f64 / self.total_tokens as f64) * 100.0
        }
    }

    /// Returns whether the account has any active tokens.
    pub fn has_active_tokens(&self) -> bool {
        self.active_tokens > 0
    }

    /// Returns whether there are expired tokens that could be cleaned up.
    pub fn has_expired_tokens(&self) -> bool {
        self.expired_tokens > 0
    }
}
