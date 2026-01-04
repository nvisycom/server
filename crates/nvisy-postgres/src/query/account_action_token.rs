//! Account action token repository for managing action token database operations.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{AccountActionToken, NewAccountActionToken, UpdateAccountActionToken};
use crate::types::{ActionTokenType, OffsetPagination};
use crate::{PgConnection, PgError, PgResult, schema};

/// Repository for account action token database operations.
///
/// Handles temporary action tokens for password resets, email verification, and other
/// time-sensitive operations with expiration tracking.
pub trait AccountActionTokenRepository {
    /// Creates a new account action token.
    fn create_account_action_token(
        &mut self,
        new_token: NewAccountActionToken,
    ) -> impl Future<Output = PgResult<AccountActionToken>> + Send;

    /// Finds a valid account action token by UUID and action type.
    ///
    /// Only returns unused, unexpired tokens matching both criteria.
    fn find_account_action_token(
        &mut self,
        token_uuid: Uuid,
        action: ActionTokenType,
    ) -> impl Future<Output = PgResult<Option<AccountActionToken>>> + Send;

    /// Finds the most recent valid account action token for an account and action type.
    fn find_account_action_token_by_account(
        &mut self,
        account_id: Uuid,
        action: ActionTokenType,
    ) -> impl Future<Output = PgResult<Option<AccountActionToken>>> + Send;

    /// Updates an account action token.
    fn update_account_action_token(
        &mut self,
        token_uuid: Uuid,
        updates: UpdateAccountActionToken,
    ) -> impl Future<Output = PgResult<AccountActionToken>> + Send;

    /// Marks an account action token as used.
    fn use_account_action_token(
        &mut self,
        token_uuid: Uuid,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<AccountActionToken>> + Send;

    /// Invalidates an account action token by marking it as used.
    fn invalidate_account_action_token(
        &mut self,
        token_uuid: Uuid,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Lists account action tokens for an account with optional used filter.
    fn offset_list_account_action_tokens(
        &mut self,
        account_id: Uuid,
        include_used: bool,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<AccountActionToken>>> + Send;

    /// Lists account action tokens by type with filtering options.
    fn offset_list_account_action_tokens_by_type(
        &mut self,
        action: ActionTokenType,
        include_used: bool,
        include_expired: bool,
        pagination: OffsetPagination,
    ) -> impl Future<Output = PgResult<Vec<AccountActionToken>>> + Send;

    /// Invalidates all unused account action tokens for an account.
    fn invalidate_all_account_action_tokens(
        &mut self,
        account_id: Uuid,
        action: Option<ActionTokenType>,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Deletes expired and used account action tokens.
    fn cleanup_expired_account_action_tokens(
        &mut self,
        account_id: Option<Uuid>,
    ) -> impl Future<Output = PgResult<i64>> + Send;
}

impl AccountActionTokenRepository for PgConnection {
    async fn create_account_action_token(
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

    async fn find_account_action_token(
        &mut self,
        token_uuid: Uuid,
        action: ActionTokenType,
    ) -> PgResult<Option<AccountActionToken>> {
        use diesel::dsl::now;
        use schema::account_action_tokens::{self, dsl};

        account_action_tokens::table
            .filter(dsl::action_token.eq(token_uuid))
            .filter(dsl::action_type.eq(action))
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.gt(now))
            .select(AccountActionToken::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn find_account_action_token_by_account(
        &mut self,
        account_id: Uuid,
        action: ActionTokenType,
    ) -> PgResult<Option<AccountActionToken>> {
        use diesel::dsl::now;
        use schema::account_action_tokens::{self, dsl};

        account_action_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::action_type.eq(action))
            .filter(dsl::used_at.is_null())
            .filter(dsl::expired_at.gt(now))
            .order(dsl::issued_at.desc())
            .select(AccountActionToken::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn update_account_action_token(
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

    async fn use_account_action_token(
        &mut self,
        token_uuid: Uuid,
        account_id: Uuid,
    ) -> PgResult<AccountActionToken> {
        use diesel::dsl::now;
        use schema::account_action_tokens::{self, dsl};

        diesel::update(
            account_action_tokens::table
                .filter(dsl::action_token.eq(token_uuid))
                .filter(dsl::account_id.eq(account_id)),
        )
        .set(dsl::used_at.eq(now))
        .returning(AccountActionToken::as_returning())
        .get_result(self)
        .await
        .map_err(PgError::from)
    }

    async fn invalidate_account_action_token(&mut self, token_uuid: Uuid) -> PgResult<bool> {
        use diesel::dsl::now;
        use schema::account_action_tokens::{self, dsl};

        let rows_affected =
            diesel::update(account_action_tokens::table.filter(dsl::action_token.eq(token_uuid)))
                .set(dsl::used_at.eq(now))
                .execute(self)
                .await
                .map_err(PgError::from)?;

        Ok(rows_affected > 0)
    }

    async fn offset_list_account_action_tokens(
        &mut self,
        account_id: Uuid,
        include_used: bool,
        pagination: OffsetPagination,
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

    async fn offset_list_account_action_tokens_by_type(
        &mut self,
        action: ActionTokenType,
        include_used: bool,
        include_expired: bool,
        pagination: OffsetPagination,
    ) -> PgResult<Vec<AccountActionToken>> {
        use diesel::dsl::now;
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
            query = query.filter(dsl::expired_at.gt(now));
        }

        query.load(self).await.map_err(PgError::from)
    }

    async fn invalidate_all_account_action_tokens(
        &mut self,
        account_id: Uuid,
        action: Option<ActionTokenType>,
    ) -> PgResult<i64> {
        use diesel::dsl::now;
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
            .set(dsl::used_at.eq(now))
            .execute(self)
            .await
            .map_err(PgError::from)
            .map(|rows| rows as i64)
    }

    async fn cleanup_expired_account_action_tokens(
        &mut self,
        account_id: Option<Uuid>,
    ) -> PgResult<i64> {
        use diesel::dsl::now;
        use schema::account_action_tokens::{self, dsl};

        let mut query = diesel::delete(
            account_action_tokens::table
                .filter(dsl::expired_at.lt(now).or(dsl::used_at.is_not_null())),
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
