//! Account API token repository for managing API tokens.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use crate::model::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
use crate::types::{ApiTokenType, CursorPage, CursorPagination, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for account API token database operations.
///
/// Handles long-lived API tokens for programmatic access with support for
/// expiration tracking and cleanup operations.
pub trait AccountApiTokenRepository {
    /// Creates a new account API token.
    fn create_account_api_token(
        &mut self,
        new_token: NewAccountApiToken,
    ) -> impl Future<Output = PgResult<AccountApiToken>> + Send;

    /// Finds an account API token by its ID.
    fn find_account_api_token_by_id(
        &mut self,
        token_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<AccountApiToken>>> + Send;

    /// Updates an account API token.
    fn update_account_api_token(
        &mut self,
        token_id: Uuid,
        updates: UpdateAccountApiToken,
    ) -> impl Future<Output = PgResult<AccountApiToken>> + Send;

    /// Updates the account API token's last used timestamp.
    fn touch_account_api_token(
        &mut self,
        token_id: Uuid,
    ) -> impl Future<Output = PgResult<AccountApiToken>> + Send;

    /// Soft deletes an account API token.
    fn delete_account_api_token(
        &mut self,
        token_id: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Soft deletes all account API tokens for an account.
    fn delete_all_account_api_tokens(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Soft deletes account API tokens by type with optional exceptions.
    fn delete_account_api_tokens_by_type(
        &mut self,
        account_id: Uuid,
        token_type: ApiTokenType,
        except_ids: &[Uuid],
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Lists active, unexpired account API tokens with offset pagination.
    fn offset_list_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<AccountApiToken>>> + Send;

    /// Lists active, unexpired account API tokens with cursor pagination.
    fn cursor_list_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = PgResult<CursorPage<AccountApiToken>>> + Send;

    /// Lists all non-deleted account API tokens including expired ones.
    fn offset_list_all_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<AccountApiToken>>> + Send;

    /// Soft-deletes all expired account API tokens system-wide.
    fn cleanup_expired_account_api_tokens(&mut self) -> impl Future<Output = PgResult<i64>> + Send;
}

impl AccountApiTokenRepository for PgConnection {
    async fn create_account_api_token(
        &mut self,
        new_token: NewAccountApiToken,
    ) -> PgResult<AccountApiToken> {
        use schema::account_api_tokens;

        diesel::insert_into(account_api_tokens::table)
            .values(&new_token)
            .returning(AccountApiToken::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_account_api_token_by_id(
        &mut self,
        token_id: Uuid,
    ) -> PgResult<Option<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::id.eq(token_id))
            .filter(dsl::deleted_at.is_null())
            .select(AccountApiToken::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn update_account_api_token(
        &mut self,
        token_id: Uuid,
        updates: UpdateAccountApiToken,
    ) -> PgResult<AccountApiToken> {
        use schema::account_api_tokens::{self, dsl};

        diesel::update(
            account_api_tokens::table
                .filter(dsl::id.eq(token_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(&updates)
        .returning(AccountApiToken::as_returning())
        .get_result(self)
        .await
        .map_err(PgError::from)
    }

    async fn touch_account_api_token(&mut self, token_id: Uuid) -> PgResult<AccountApiToken> {
        self.update_account_api_token(
            token_id,
            UpdateAccountApiToken {
                last_used_at: Some(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
                ..Default::default()
            },
        )
        .await
    }

    async fn delete_account_api_token(&mut self, token_id: Uuid) -> PgResult<bool> {
        use diesel::dsl::now;
        use schema::account_api_tokens::{self, dsl};

        let rows_affected = diesel::update(account_api_tokens::table.filter(dsl::id.eq(token_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(rows_affected > 0)
    }

    async fn delete_all_account_api_tokens(&mut self, account_id: Uuid) -> PgResult<i64> {
        use diesel::dsl::now;
        use schema::account_api_tokens::{self, dsl};

        diesel::update(
            account_api_tokens::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(now))
        .execute(self)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }

    async fn delete_account_api_tokens_by_type(
        &mut self,
        account_id: Uuid,
        token_type: ApiTokenType,
        except_ids: &[Uuid],
    ) -> PgResult<i64> {
        use diesel::dsl::now;
        use schema::account_api_tokens::{self, dsl};

        let mut query = diesel::update(
            account_api_tokens::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::session_type.eq(token_type))
                .filter(dsl::deleted_at.is_null()),
        )
        .into_boxed();

        if !except_ids.is_empty() {
            query = query.filter(dsl::id.ne_all(except_ids));
        }

        query
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }

    async fn offset_list_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<AccountApiToken>> {
        use diesel::dsl::now;
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::expired_at.is_null().or(dsl::expired_at.gt(now)))
            .order(dsl::issued_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountApiToken::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn cursor_list_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> PgResult<CursorPage<AccountApiToken>> {
        use diesel::dsl::{count_star, now};
        use schema::account_api_tokens::{self, dsl};

        let base_filter = dsl::account_id
            .eq(account_id)
            .and(dsl::deleted_at.is_null())
            .and(dsl::expired_at.is_null().or(dsl::expired_at.gt(now)));

        let total = if pagination.include_count {
            Some(
                account_api_tokens::table
                    .filter(base_filter)
                    .select(count_star())
                    .get_result(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let items = if let Some(cursor) = &pagination.after {
            let cursor_ts = jiff_diesel::Timestamp::from(cursor.timestamp);
            account_api_tokens::table
                .filter(base_filter)
                .filter(
                    dsl::issued_at
                        .lt(cursor_ts)
                        .or(dsl::issued_at.eq(cursor_ts).and(dsl::id.lt(cursor.id))),
                )
                .order((dsl::issued_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(AccountApiToken::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        } else {
            account_api_tokens::table
                .filter(base_filter)
                .order((dsl::issued_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(AccountApiToken::as_select())
                .load(self)
                .await
                .map_err(PgError::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |t| {
            (t.issued_at.into(), t.id)
        }))
    }

    async fn offset_list_all_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::issued_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountApiToken::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn cleanup_expired_account_api_tokens(&mut self) -> PgResult<i64> {
        use diesel::dsl::now;
        use schema::account_api_tokens::{self, dsl};

        diesel::update(
            account_api_tokens::table
                .filter(dsl::expired_at.is_not_null())
                .filter(dsl::expired_at.lt(now))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(now))
        .execute(self)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }
}
