//! Account API token repository for managing API tokens.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jiff::{Span, Timestamp};
use uuid::Uuid;

use super::Pagination;
use crate::model::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
use crate::{PgError, PgResult, schema};
use crate::PgConnection;

/// Repository for account API token database operations.
///
/// Handles long-lived API tokens for programmatic access with support for token
/// refresh, expiration tracking, and cleanup operations.
pub trait AccountApiTokenRepository {
    /// Creates a new API token for programmatic access.
    fn create_token(
        &mut self,
        new_token: NewAccountApiToken,
    ) -> impl Future<Output = PgResult<AccountApiToken>> + Send;

    /// Finds an active token by its access token sequence.
    fn find_token_by_access_token(
        &mut self,
        access_token: Uuid,
    ) -> impl Future<Output = PgResult<Option<AccountApiToken>>> + Send;

    /// Finds an active token by its refresh token sequence.
    fn find_token_by_refresh_token(
        &mut self,
        refresh_token: Uuid,
    ) -> impl Future<Output = PgResult<Option<AccountApiToken>>> + Send;

    /// Updates a token's properties with new values.
    fn update_token(
        &mut self,
        access_token: Uuid,
        updates: UpdateAccountApiToken,
    ) -> impl Future<Output = PgResult<AccountApiToken>> + Send;

    /// Updates the token's last used timestamp.
    fn touch_token(
        &mut self,
        access_token: Uuid,
    ) -> impl Future<Output = PgResult<AccountApiToken>> + Send;

    /// Refreshes a token by generating new access and refresh sequences.
    ///
    /// Creates new UUIDs for both sequences and extends expiration by 7 days.
    fn refresh_token(
        &mut self,
        refresh_token: Uuid,
    ) -> impl Future<Output = PgResult<AccountApiToken>> + Send;

    /// Soft deletes a token. Returns true if deleted, false if not found.
    fn delete_token(&mut self, access_token: Uuid) -> impl Future<Output = PgResult<bool>> + Send;

    /// Soft deletes all active tokens for an account.
    ///
    /// Returns the count of deleted tokens.
    fn delete_all_tokens_for_account(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = PgResult<i64>> + Send;

    /// Lists active, unexpired tokens for an account.
    fn list_account_tokens(
        &mut self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<AccountApiToken>>> + Send;

    /// Lists all non-deleted tokens for an account including expired ones.
    fn list_all_account_tokens(
        &mut self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> impl Future<Output = PgResult<Vec<AccountApiToken>>> + Send;

    /// Soft-deletes all expired tokens system-wide.
    ///
    /// Returns the count of affected tokens.
    fn cleanup_expired_tokens(&mut self) -> impl Future<Output = PgResult<i64>> + Send;
}

impl AccountApiTokenRepository for PgConnection {
    async fn create_token(&mut self, new_token: NewAccountApiToken) -> PgResult<AccountApiToken> {

        use schema::account_api_tokens;

        diesel::insert_into(account_api_tokens::table)
            .values(&new_token)
            .returning(AccountApiToken::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn find_token_by_access_token(
        &mut self,
        access_token: Uuid,
    ) -> PgResult<Option<AccountApiToken>> {

        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::access_seq.eq(access_token))
            .filter(dsl::deleted_at.is_null())
            .select(AccountApiToken::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn find_token_by_refresh_token(
        &mut self,
        refresh_token: Uuid,
    ) -> PgResult<Option<AccountApiToken>> {

        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::refresh_seq.eq(refresh_token))
            .filter(dsl::deleted_at.is_null())
            .select(AccountApiToken::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)
    }

    async fn update_token(
        &mut self,
        access_token: Uuid,
        updates: UpdateAccountApiToken,
    ) -> PgResult<AccountApiToken> {

        use schema::account_api_tokens::{self, dsl};

        diesel::update(account_api_tokens::table.filter(dsl::access_seq.eq(access_token)))
            .set(&updates)
            .returning(AccountApiToken::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn touch_token(&mut self, access_token: Uuid) -> PgResult<AccountApiToken> {
        self.update_token(
            access_token,
            UpdateAccountApiToken {
                last_used_at: Some(jiff_diesel::Timestamp::from(Timestamp::now())),
                ..Default::default()
            },
        )
        .await
    }

    async fn refresh_token(&mut self, refresh_token: Uuid) -> PgResult<AccountApiToken> {

        use schema::account_api_tokens::{self, dsl};

        let new_access_seq = Uuid::new_v4();
        let new_refresh_seq = Uuid::new_v4();
        let new_expiry = jiff_diesel::Timestamp::from(Timestamp::now() + Span::new().days(7));

        diesel::update(account_api_tokens::table.filter(dsl::refresh_seq.eq(refresh_token)))
            .set((
                dsl::access_seq.eq(new_access_seq),
                dsl::refresh_seq.eq(new_refresh_seq),
                dsl::expired_at.eq(new_expiry),
                dsl::last_used_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))),
            ))
            .returning(AccountApiToken::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)
    }

    async fn delete_token(&mut self, access_token: Uuid) -> PgResult<bool> {

        use schema::account_api_tokens::{self, dsl};

        let rows_affected =
            diesel::update(account_api_tokens::table.filter(dsl::access_seq.eq(access_token)))
                .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
                .execute(self)
                .await
                .map_err(PgError::from)?;

        Ok(rows_affected > 0)
    }

    async fn delete_all_tokens_for_account(&mut self, account_id: Uuid) -> PgResult<i64> {

        use schema::account_api_tokens::{self, dsl};

        diesel::update(
            account_api_tokens::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
        .execute(self)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }

    async fn list_account_tokens(
        &mut self,
        account_id: Uuid,
        pagination: Pagination,
    ) -> PgResult<Vec<AccountApiToken>> {

        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::deleted_at.is_null())
            .filter(dsl::expired_at.gt(jiff_diesel::Timestamp::from(Timestamp::now())))
            .order(dsl::issued_at.desc())
            .limit(pagination.limit)
            .offset(pagination.offset)
            .select(AccountApiToken::as_select())
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn list_all_account_tokens(
        &mut self,
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
            .load(self)
            .await
            .map_err(PgError::from)
    }

    async fn cleanup_expired_tokens(&mut self) -> PgResult<i64> {

        use schema::account_api_tokens::{self, dsl};

        diesel::update(
            account_api_tokens::table
                .filter(dsl::expired_at.lt(jiff_diesel::Timestamp::from(Timestamp::now())))
                .filter(dsl::deleted_at.is_null()),
        )
        .set(dsl::deleted_at.eq(Some(jiff_diesel::Timestamp::from(Timestamp::now()))))
        .execute(self)
        .await
        .map_err(PgError::from)
        .map(|rows| rows as i64)
    }
}
