//! Account API token repository for managing API tokens.

use std::future::Future;
use std::time::Duration;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
use crate::types::{ApiTokenType, CursorPage, CursorPagination, OffsetPagination, session};
use crate::{Error, PgConnection, Result, schema};

/// Repository for account API token database operations.
///
/// Handles long-lived API tokens for programmatic access with support for
/// expiration tracking and cleanup operations.
pub trait AccountApiTokenRepository {
    /// Creates a new account API token.
    fn create_account_api_token(
        &mut self,
        new_token: NewAccountApiToken,
    ) -> impl Future<Output = Result<AccountApiToken>> + Send;

    /// Finds an account API token by its ID.
    fn find_account_api_token_by_id(
        &mut self,
        token_id: Uuid,
    ) -> impl Future<Output = Result<Option<AccountApiToken>>> + Send;

    /// Returns whether the session token is still valid: it exists, belongs to
    /// the given account, has not been revoked (soft-deleted), and is within its
    /// idle bound (`expired_at`). For `web` browser sessions it must additionally
    /// be within the absolute age cap (`issued_at + max_age`); programmatic
    /// `api` tokens are long-lived and exempt from that cap, governed only by
    /// their own `expired_at`.
    ///
    /// This is the **sole authority** for a session's validity, checked on every
    /// authenticated request. Revocation, idle expiry, and (for `web`) absolute
    /// expiry all go through here — the bearer JWT is only a signed pointer to the
    /// row. The authentication path treats any error from this as a failure (fail
    /// closed); callers must never add an allow-on-error fallback, or a revoked or
    /// expired session would authenticate.
    fn account_api_token_is_active(
        &mut self,
        token_id: Uuid,
        account_id: Uuid,
        max_age: Duration,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Slides a `web` browser session's idle bound (`expired_at`) forward on use,
    /// and records `last_used_at`, throttled so a burst of requests writes at most
    /// once per `throttle` interval.
    ///
    /// Only `web` sessions slide: programmatic `api` tokens carry a
    /// user-chosen `expired_at` set at creation, which is authoritative and must
    /// not be overwritten by a browser-session window — the update is a no-op for
    /// them.
    ///
    /// The new idle bound is `now + idle`, clamped so it never exceeds the
    /// absolute cap (`issued_at + max_age`) — an actively used session slides, but
    /// never past its hard age limit. The idle window is chosen from the row's own
    /// `is_remembered`, so the slide re-applies the same window the session was
    /// minted with. The write is skipped when `last_used_at` is newer than
    /// `throttle` ago, so it does not fire on every request.
    ///
    /// Returns whether a row was actually updated (informational: `false` means
    /// throttled, not a `web` session, or the row was gone). This is best-effort
    /// keep-alive, never an authentication gate — validity is decided by
    /// [`account_api_token_is_active`](Self::account_api_token_is_active).
    fn slide_account_api_token(
        &mut self,
        token_id: Uuid,
        window: session::SlidingWindow,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Updates an account API token.
    fn update_account_api_token(
        &mut self,
        token_id: Uuid,
        updates: UpdateAccountApiToken,
    ) -> impl Future<Output = Result<AccountApiToken>> + Send;

    /// Soft deletes an account API token.
    fn delete_account_api_token(
        &mut self,
        token_id: Uuid,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Soft deletes all account API tokens for an account.
    fn delete_all_account_api_tokens(
        &mut self,
        account_id: Uuid,
    ) -> impl Future<Output = Result<i64>> + Send;

    /// Soft deletes account API tokens by type with optional exceptions.
    fn delete_account_api_tokens_by_type(
        &mut self,
        account_id: Uuid,
        token_type: ApiTokenType,
        except_ids: &[Uuid],
    ) -> impl Future<Output = Result<i64>> + Send;

    /// Caps the number of live (`deleted_at IS NULL`) `app` session tokens for an
    /// account to the `keep` newest by `issued_at`, soft-deleting the rest.
    ///
    /// Called after minting a new `app` (desktop) token so repeated desktop logins
    /// do not accumulate unbounded long-lived credentials — the oldest sessions
    /// are evicted, keeping at most `keep` per account. Returns how many were
    /// revoked.
    fn prune_app_tokens(
        &mut self,
        account_id: Uuid,
        keep: usize,
    ) -> impl Future<Output = Result<i64>> + Send;

    /// Lists active, unexpired account API tokens with offset pagination.
    fn offset_list_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<AccountApiToken>>> + Send;

    /// Lists active, unexpired account API tokens with cursor pagination.
    fn cursor_list_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> impl Future<Output = Result<CursorPage<AccountApiToken>>> + Send;

    /// Lists all non-deleted account API tokens including expired ones.
    fn offset_list_all_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> impl Future<Output = Result<Vec<AccountApiToken>>> + Send;

    /// Soft-deletes all expired account API tokens system-wide.
    fn cleanup_expired_account_api_tokens(&mut self) -> impl Future<Output = Result<i64>> + Send;
}

impl AccountApiTokenRepository for PgConnection {
    async fn create_account_api_token(
        &mut self,
        new_token: NewAccountApiToken,
    ) -> Result<AccountApiToken> {
        use schema::account_api_tokens;

        diesel::insert_into(account_api_tokens::table)
            .values(&new_token)
            .returning(AccountApiToken::as_returning())
            .get_result(self)
            .await
            .map_err(Error::from)
    }

    async fn find_account_api_token_by_id(
        &mut self,
        token_id: Uuid,
    ) -> Result<Option<AccountApiToken>> {
        use schema::account_api_tokens::{self, dsl};

        account_api_tokens::table
            .filter(dsl::id.eq(token_id))
            .filter(dsl::deleted_at.is_null())
            .select(AccountApiToken::as_select())
            .first(self)
            .await
            .optional()
            .map_err(Error::from)
    }

    async fn account_api_token_is_active(
        &mut self,
        token_id: Uuid,
        account_id: Uuid,
        max_age: Duration,
    ) -> Result<bool> {
        use diesel::dsl::{exists, now, select};
        use diesel::sql_types::{BigInt, Timestamptz};
        use schema::account_api_tokens::{self, dsl};

        // Time bounds are evaluated against the database clock (`now()`), not the
        // application's, so validity does not depend on server clock skew.
        //   - idle bound:   `expired_at IS NULL OR expired_at > now()` (all tokens)
        //   - absolute cap: `issued_at > now() - max_age`, applied ONLY to `web`
        //     browser sessions. The absolute cap is a browser-session policy;
        //     programmatic `api` tokens are long-lived and authoritative on
        //     their own `expired_at`, so they are exempt from it.
        let max_age_secs = max_age.as_secs() as i64;
        let age_cutoff = diesel::dsl::sql::<Timestamptz>("now() - (")
            .bind::<BigInt, _>(max_age_secs)
            .sql(" * interval '1 second')");

        select(exists(
            account_api_tokens::table
                .filter(dsl::id.eq(token_id))
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::expired_at.is_null().or(dsl::expired_at.gt(now)))
                .filter(
                    dsl::session_type
                        .ne(ApiTokenType::Web)
                        .or(dsl::issued_at.gt(age_cutoff)),
                ),
        ))
        .get_result(self)
        .await
        .map_err(Error::from)
    }

    async fn slide_account_api_token(
        &mut self,
        token_id: Uuid,
        window: session::SlidingWindow,
    ) -> Result<bool> {
        use diesel::sql_types::{BigInt, Nullable, Timestamptz};
        use schema::account_api_tokens::{self, dsl};

        let idle_remembered_secs = window.idle_remembered.as_secs() as i64;
        let idle_default_secs = window.idle_default.as_secs() as i64;
        let max_age_secs = window.max_age.as_secs() as i64;
        let throttle_secs = window.throttle.as_secs() as i64;

        // New idle bound, chosen from the row's own `is_remembered` and clamped to
        // the absolute cap so a slide can never push a session past
        // `issued_at + max_age`. All arithmetic is on the database clock. Typed
        // `Nullable` to assign the nullable `expired_at` column.
        let new_expired_at = diesel::dsl::sql::<Nullable<Timestamptz>>("LEAST(now() + ((")
            .sql("CASE WHEN is_remembered THEN ")
            .bind::<BigInt, _>(idle_remembered_secs)
            .sql(" ELSE ")
            .bind::<BigInt, _>(idle_default_secs)
            .sql(" END) * interval '1 second'), issued_at + (")
            .bind::<BigInt, _>(max_age_secs)
            .sql(" * interval '1 second'))");

        // Throttle: only slide when `last_used_at` is null or older than the
        // throttle interval, so a burst of requests writes at most once per
        // interval. `deleted_at IS NULL` keeps a revoked row from being revived.
        // Typed `Nullable` to compare against the nullable `last_used_at` column.
        let throttle_cutoff = diesel::dsl::sql::<Nullable<Timestamptz>>("now() - (")
            .bind::<BigInt, _>(throttle_secs)
            .sql(" * interval '1 second')");

        // Only `web` browser sessions slide. Programmatic `api` tokens carry
        // a user-chosen `expired_at` set at creation that is authoritative, so
        // sliding must not overwrite it with a short browser-session window.
        let rows = diesel::update(
            account_api_tokens::table
                .filter(dsl::id.eq(token_id))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::session_type.eq(ApiTokenType::Web))
                .filter(
                    dsl::last_used_at
                        .is_null()
                        .or(dsl::last_used_at.lt(throttle_cutoff)),
                ),
        )
        .set((
            dsl::expired_at.eq(new_expired_at),
            dsl::last_used_at.eq(diesel::dsl::now),
        ))
        .execute(self)
        .await
        .map_err(Error::from)?;

        Ok(rows > 0)
    }

    async fn update_account_api_token(
        &mut self,
        token_id: Uuid,
        updates: UpdateAccountApiToken,
    ) -> Result<AccountApiToken> {
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
        .map_err(Error::from)
    }

    async fn delete_account_api_token(&mut self, token_id: Uuid) -> Result<bool> {
        use diesel::dsl::now;
        use schema::account_api_tokens::{self, dsl};

        let rows_affected = diesel::update(account_api_tokens::table.filter(dsl::id.eq(token_id)))
            .set(dsl::deleted_at.eq(now))
            .execute(self)
            .await
            .map_err(Error::from)?;

        Ok(rows_affected > 0)
    }

    async fn delete_all_account_api_tokens(&mut self, account_id: Uuid) -> Result<i64> {
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
        .map_err(Error::from)
        .map(|rows| rows as i64)
    }

    async fn delete_account_api_tokens_by_type(
        &mut self,
        account_id: Uuid,
        token_type: ApiTokenType,
        except_ids: &[Uuid],
    ) -> Result<i64> {
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
            .map_err(Error::from)
            .map(|rows| rows as i64)
    }

    async fn prune_app_tokens(&mut self, account_id: Uuid, keep: usize) -> Result<i64> {
        use diesel::dsl::now;
        use schema::account_api_tokens::{self, dsl};

        // The `keep` newest live `app` tokens for the account are retained; load
        // their ids, then soft-delete every other live `app` token. Two steps
        // rather than a `NOT IN (… LIMIT …)` subquery, which Diesel does not
        // express cleanly.
        let keep_ids: Vec<Uuid> = account_api_tokens::table
            .filter(dsl::account_id.eq(account_id))
            .filter(dsl::session_type.eq(ApiTokenType::App))
            .filter(dsl::deleted_at.is_null())
            .order(dsl::issued_at.desc())
            .limit(keep as i64)
            .select(dsl::id)
            .load(self)
            .await
            .map_err(Error::from)?;

        diesel::update(
            account_api_tokens::table
                .filter(dsl::account_id.eq(account_id))
                .filter(dsl::session_type.eq(ApiTokenType::App))
                .filter(dsl::deleted_at.is_null())
                .filter(dsl::id.ne_all(&keep_ids)),
        )
        .set(dsl::deleted_at.eq(now))
        .execute(self)
        .await
        .map_err(Error::from)
        .map(|rows| rows as i64)
    }

    async fn offset_list_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> Result<Vec<AccountApiToken>> {
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
            .map_err(Error::from)
    }

    async fn cursor_list_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: CursorPagination,
    ) -> Result<CursorPage<AccountApiToken>> {
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
                    .map_err(Error::from)?,
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
                .map_err(Error::from)?
        } else {
            account_api_tokens::table
                .filter(base_filter)
                .order((dsl::issued_at.desc(), dsl::id.desc()))
                .limit(pagination.fetch_limit())
                .select(AccountApiToken::as_select())
                .load(self)
                .await
                .map_err(Error::from)?
        };

        Ok(CursorPage::new(items, total, pagination.limit, |t| {
            (t.issued_at.into(), t.id)
        }))
    }

    async fn offset_list_all_account_api_tokens(
        &mut self,
        account_id: Uuid,
        pagination: OffsetPagination,
    ) -> Result<Vec<AccountApiToken>> {
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
            .map_err(Error::from)
    }

    async fn cleanup_expired_account_api_tokens(&mut self) -> Result<i64> {
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
        .map_err(Error::from)
        .map(|rows| rows as i64)
    }
}
