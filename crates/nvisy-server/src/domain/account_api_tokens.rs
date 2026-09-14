//! Account API-token domain logic: create (with a signed JWT), list, read,
//! update, revoke.
//!
//! Owns the token rules a handler would otherwise inline — signing the JWT for a
//! newly created token through the shared [`AuthIssuer`], the ownership check that
//! scopes every read and mutation to the acting account, and the rule that only
//! API tokens (not sessions) may be renamed.

use nvisy_postgres::model::{AccountApiToken, NewAccountApiToken, UpdateAccountApiToken};
use nvisy_postgres::query::{AccountApiTokenRepository, AccountRepository, ApiTokenCursor};
use nvisy_postgres::types::{ApiTokenType, CursorPage, CursorPagination};
use nvisy_postgres::{AsyncConnection, PgClient};
use uuid::Uuid;

use crate::domain::output::CreatedApiToken;
use crate::response::{Error, ErrorKind, Result};
use crate::service::AuthIssuer;

/// Tracing target for account API-token domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::account";

/// Creates, reads, updates, and revokes an account's API tokens.
///
/// Holds the Postgres client (own-connection-per-call) and the auth issuer (to
/// sign the JWT for a new token). Resolved per request from
/// [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct AccountApiTokenService {
    postgres: PgClient,
    issuer: AuthIssuer,
}

impl AccountApiTokenService {
    /// Creates an [`AccountApiTokenService`] over its clients.
    #[must_use]
    pub fn new(postgres: PgClient, issuer: AuthIssuer) -> Self {
        Self { postgres, issuer }
    }

    /// Creates an API token and signs its one-time JWT.
    ///
    /// The JWT is returned only here, at creation; it is never stored or shown
    /// again.
    ///
    /// # Errors
    ///
    /// - `NotFound` if no account has the given id.
    /// - An auth error if signing the token's JWT fails.
    /// - A database error if the connection or transaction fails.
    pub async fn create(
        &self,
        account_id: Uuid,
        new_token: NewAccountApiToken,
    ) -> Result<CreatedApiToken> {
        let mut conn = self.postgres.get_connection().await?;

        // The JWT claims are built from the account, so it must exist.
        let account = conn
            .find_account_by_id(account_id)
            .await?
            .ok_or_else(|| Error::not_found("account"))?;

        // Insert the row and sign its JWT in one transaction: signing consumes the
        // persisted token's id/claims, so if it fails the insert is rolled back and
        // no unusable, un-signed token row is left behind.
        let issuer = &self.issuer;
        let (token, jwt) = conn
            .transaction(async |conn| {
                let token = conn.create_account_api_token(new_token).await?;
                let jwt = issuer.sign(&account, &token)?;
                Ok::<_, Error>((token, jwt))
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, token_id = %token.id, "API token created");
        Ok(CreatedApiToken { token, jwt })
    }

    /// Lists the account's API tokens with cursor pagination.
    ///
    /// # Errors
    ///
    /// - A database error if the connection or query fails.
    pub async fn list(
        &self,
        account_id: Uuid,
        pagination: CursorPagination<ApiTokenCursor>,
    ) -> Result<CursorPage<AccountApiToken>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_account_api_tokens(account_id, pagination)
            .await?)
    }

    /// Reads one of the account's API tokens by id, or a `NotFound`.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the token does not exist or belongs to another account.
    /// - A database error if the connection or query fails.
    pub async fn read(&self, account_id: Uuid, token_id: Uuid) -> Result<AccountApiToken> {
        let mut conn = self.postgres.get_connection().await?;
        find_account_token(&mut conn, account_id, token_id).await
    }

    /// Renames an API token.
    ///
    /// Refuses (403) to rename a session token — only API tokens carry a
    /// user-facing name.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the token does not exist or belongs to another account.
    /// - `Forbidden` if the token is a session token rather than an API token.
    /// - A database error if the connection or query fails.
    pub async fn update(
        &self,
        account_id: Uuid,
        token_id: Uuid,
        display_name: Option<String>,
    ) -> Result<AccountApiToken> {
        let mut conn = self.postgres.get_connection().await?;
        let token = find_account_token(&mut conn, account_id, token_id).await?;

        if token.session_type != ApiTokenType::Api {
            return Err(ErrorKind::Forbidden.with_message("Only API tokens can be renamed"));
        }

        let updated = conn
            .update_account_api_token(
                token.id,
                UpdateAccountApiToken {
                    display_name,
                    ..Default::default()
                },
            )
            .await?;
        tracing::info!(target: TRACING_TARGET, "API token updated");
        Ok(updated)
    }

    /// Revokes (soft-deletes) an API token, or a 400 if it was already revoked.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the token does not exist or belongs to another account.
    /// - `BadRequest` if the token was already revoked.
    /// - A database error if the connection or query fails.
    pub async fn revoke(&self, account_id: Uuid, token_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        let token = find_account_token(&mut conn, account_id, token_id).await?;

        let deleted = conn.delete_account_api_token(token.id).await?;
        if !deleted {
            return Err(ErrorKind::BadRequest.with_message("API token is already revoked"));
        }

        tracing::info!(target: TRACING_TARGET, "API token revoked");
        Ok(())
    }
}

/// Finds an API token by id and verifies it belongs to the account. A missing
/// token, or one owned by another account, is reported as not-found — there is no
/// cross-account probe.
async fn find_account_token(
    conn: &mut nvisy_postgres::PgConn,
    account_id: Uuid,
    token_id: Uuid,
) -> Result<AccountApiToken> {
    let token = conn
        .find_account_api_token_by_id(token_id)
        .await?
        .filter(|token| token.account_id == account_id)
        .ok_or_else(|| ErrorKind::NotFound.with_message("API token not found"))?;
    Ok(token)
}
