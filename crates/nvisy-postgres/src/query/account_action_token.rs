//! Account action token repository for managing action token database operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::Timestamp;
use uuid::Uuid;

use super::Pagination;
use crate::model::{AccountActionToken, NewAccountActionToken, UpdateAccountActionToken};
use crate::types::ActionTokenType;
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for account action token database operations.
///
/// Handles temporary action tokens for password resets, email verification, and other
/// time-sensitive operations with expiration and attempt tracking.
pub trait AccountActionTokenRepository {
    /// Creates a new action token for the specified account.
    fn create_token(
        &mut self,
        new_token: NewAccountActionToken,
    ) -> impl Future<Output = PgResult<AccountActionToken>> + Send;

    /// Finds a valid token by UUID and action type.
    ///
    /// Only returns unused, unexpired tokens matching both criteria.
    fn find_token(
        &mut self,
        token_uuid: Uuid,
        action: ActionTokenType,
    ) -> impl Future<Output = PgResult<Option<AccountActionToken>>> + Send;

    /// Finds the most recent valid token for an account and action type.
    fn find_account_token(
        &mut self,
        account_id: Uuid,
        action: ActionTokenType,
    ) -> impl Future<Output = PgResult<Option<AccountActionToken>>> + Send;

    /// Updates a token's properties with new values.
    fn update_token(
        &mut self,
        token_uuid: Uuid,
        updates: UpdateAccountActionToken,
    ) -> impl Future<Output = PgResult<AccountActionToken>> + Send;

    /// Increments the attempt count after a failed validation.
    fn increment_token_attempts(
        &mut self,
        token_uuid: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<AccountActionToken>> + Send;

    /// Marks a token as used after successful action completion.
    fn use_token(
        &mut self,
        token_uuid: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<AccountActionToken>> + Send;

    /// Invalidates a token by marking it as used.
    ///
    /// Returns true if a token was invalidated, false if not found.
    fn invalidate_token(&mut self, token_uuid: Uuid)
    -> impl Future<Output = PgResult<bool>> + Send;

    /// Lists tokens for a specific account with optional used filter.
    fn list_account_tokens(
        &mut self,
        account_id: Uuid,
        include_used: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<AccountActionToken>>> + Send;

    /// Lists tokens filtered by action type with comprehensive filtering.
    fn list_tokens_by_action(
        &mut self,
        action: ActionTokenType,
        include_used: bool,
        include_expired: bool,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<AccountActionToken>>> + Send;

    /// Invalidates all unused tokens for an account.
    ///
    /// Optionally filters by action type. Returns count of invalidated tokens.
    fn invalidate_account_tokens(
        &mut self,
        account_id: Uuid,
        action: Option<ActionTokenType>,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Deletes expired and used tokens for cleanup.
    ///
    /// Optionally scoped to a specific account. Returns count of deleted tokens.
    fn cleanup_expired_tokens(
        &mut self,
        account_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<i64>> + Send;
}

impl AccountActionTokenRepository for PgConnection {
    async fn create_token(
        &mut self,
        new_token: NewAccountActionToken,
    ) -> PgResult<AccountActionToken> {
        use schema::account_action_tokens;

        diesel::insert_into(account_action_tokens::table)
            .values(&new_token)
            .returning(AccountActionToken::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_token(
        &mut self,
        token_uuid: Uuid,
        action: ActionTokenType,
    ) -> PgResult<Option<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        account_action_tokens::table
            .filter(dsl::action_token.eq(token_uuid))
            .filter(dsl::action_type.eq(action))
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.gt(jiff_diesel::Timestamp::from(Timestamp::now())))
            .select(AccountActionToken::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn find_account_token(
        &mut self,
        account_id: Uuid,
        action: ActionTokenType,
    ) -> PgResult<Option<AccountActionToken>> {
        use schema::account_action_tokens::{self, dsl};

        account_action_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::action_type.eq(action))
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.gt(jiff_diesel::Timestamp::from(Timestamp::now())))
            .order(dsl::issued_at.desc())
            .select(AccountActionToken::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn update_token(
        &mut self,
        token_uuid: Uuid,
        updates: UpdateAccountActionToken,
    ) -> PgResult<AccountActionToken> {
        use schema::account_action_tokens::{self, dsl};

        diesel::update(account_action_tokens::table.filter(dsl::action_token.eq(token_uuid)))
            .set(&updates)
            .returning(AccountActionToken::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn increment_token_attempts(
        &mut self,
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
        .get_result(self)
        .await
        .map_err(PgError::from)
    }

    async fn use_token(
        &mut self,
        token_uuid: Uuid,
        account_id: Uuid,
    ) -> PgResult<AccountActionToken> {
        use schema::account_action_tokens::{self, dsl};

        diesel::update(
            account_action_tokens::table
                .filter(dsl::action_token.eq(token_uuid))
                .filter(dsl::account_id.eq(account_id)),
        )
        .set(dsl::used_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
        .returning(AccountActionToken::as_returning())
        .get_result(self)
        .await
        .map_err(PgError::from)
    }

    async fn invalidate_token(&mut self, token_uuid: Uuid) -> PgResult<bool> {
        use schema::account_action_tokens::{self, dsl};

        let rows_affected =
            diesel::update(account_action_tokens::table.filter(dsl::action_token.eq(token_uuid)))
                .set(dsl::used_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
                .execute(self)
                .await
                .map_err(PgError::from)?;

        Ok(rows_affected > 0)
    }

    async fn list_account_tokens(
        &mut self,
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

        query.load(self).await.map_err(PgError::from)
    }

    async fn list_tokens_by_action(
        &mut self,
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
            query =
                query.filter(dsl::expired_at.gt(jiff_diesel::Timestamp::from(Timestamp::now())));
        }

        query.load(self).await.map_err(PgError::from)
    }

    async fn invalidate_account_tokens(
        &mut self,
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
            .set(dsl::used_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
            .execute(self)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }

    async fn cleanup_expired_tokens(&mut self, account_id: Option<Uuid>) -> PgResult<i64> {
        use schema::account_action_tokens::{self, dsl};

        let mut query = diesel::delete(
            account_action_tokens::table.filter(
                dsl::expired_at
                    .lt(jiff_diesel::Timestamp::from(Timestamp::now()))
                    .or(dsl::used_at.is_not_null()),
            ),
        )
        .into_boxed();

        if let Some(acc_id) = account_id {
            query = query.filter(dsl::account_id.eq(acc_id));
        }

        query
            .execute(self)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }
}
