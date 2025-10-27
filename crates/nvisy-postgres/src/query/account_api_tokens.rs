//! Account API token repository for managing token database operations.

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use time::OffsetDateTime;
use uuid::Uuid;

use super::Pagination;
use crate::model::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
use crate::{PgError, PgResult, schema};

/// Repository for account API token-related database operations.
#[derive(Debug, Default, Clone, Copy)]
pub struct AccountApiTokenRepository;

impl AccountApiTokenRepository {
    /// Creates a new account API token repository instance.
    pub fn new() -> Self {
        Self
    }

    // Token management methods

    /// Creates a new account API token.
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

    /// Finds a token by access token.
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

    /// Finds a token by refresh token.
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

    /// Updates a token's properties.
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

    /// Updates token last used timestamp.
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

    /// Extends a token's expiration time.
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

    /// Refreshes a token by generating new access and refresh tokens.
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

    /// Soft deletes a token (logout).
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

    /// Soft deletes all tokens for an account (logout everywhere).
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

    /// Lists active tokens for an account.
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

    /// Lists all tokens for an account (including expired).
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

    /// Gets token statistics for an account.
    pub async fn get_account_token_stats(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
    ) -> PgResult<AccountApiTokenStats> {
        use schema::account_api_tokens::{self, dsl};

        let now = OffsetDateTime::now_utc();

        // Count active tokens
        let active_tokens = account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::expired_at.gt(now))
            .count()
            .get_result::<i64>(conn)
            .await
            .map_err(PgError::from)?;

        // Count total tokens (including expired/deleted)
        let total_tokens = account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .count()
            .get_result::<i64>(conn)
            .await
            .map_err(PgError::from)?;

        // Count expired tokens
        let expired_tokens = account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::expired_at.le(now))
            .count()
            .get_result::<i64>(conn)
            .await
            .map_err(PgError::from)?;

        Ok(AccountApiTokenStats {
            active_tokens,
            expired_tokens,
            total_tokens,
        })
    }

    /// Finds tokens by account ID that are about to expire.
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

    /// Finds the most recent token for an account.
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

    /// Cleans up expired tokens by soft-deleting them.
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

    /// Hard deletes old soft-deleted tokens.
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

/// Statistics for account API tokens.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountApiTokenStats {
    /// Number of currently active tokens
    pub active_tokens: i64,
    /// Number of expired but not deleted tokens
    pub expired_tokens: i64,
    /// Total number of tokens for the account
    pub total_tokens: i64,
}

impl AccountApiTokenStats {
    /// Returns the ratio of active to total tokens as a percentage.
    pub fn activity_rate(&self) -> f64 {
        if self.total_tokens == 0 {
            100.0
        } else {
            (self.active_tokens as f64 / self.total_tokens as f64) * 100.0
        }
    }

    /// Returns whether the account has any active tokens.
    pub fn has_active_tokens(&self) -> bool {
        self.active_tokens > 0
    }

    /// Returns whether the account has expired tokens that could be cleaned up.
    pub fn has_expired_tokens(&self) -> bool {
        self.expired_tokens > 0
    }
}
