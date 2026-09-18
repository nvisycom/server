//! Shared account-reference resolution for handler responses.
//!
//! Responses expose the creating/triggering account by its immutable id plus its
//! public username (display), so handlers resolve the reference from the account
//! id they already hold.

use std::collections::HashMap;

use nvisy_postgres::PgConn;
use nvisy_postgres::query::AccountRepository;
use uuid::Uuid;

use crate::handler::response::AccountRef;
use crate::response::{ErrorKind, Result};

/// Resolves a public reference to a required account (e.g. a resource's
/// creator, or whoever triggered an action).
///
/// The account is expected to exist — typically the authenticated caller — so a
/// missing row is a server-side inconsistency rather than a client error.
pub async fn resolve_account_ref(conn: &mut PgConn, account_id: Uuid) -> Result<AccountRef> {
    conn.find_account_by_id(account_id)
        .await?
        .map(|account| {
            AccountRef::new(
                account.id,
                account.username,
                account.display_name,
                account.avatar_url,
            )
        })
        .ok_or_else(|| ErrorKind::InternalServerError.with_message("account not found"))
}

/// Resolves public references for many accounts in one query, returning a lookup
/// keyed by account id. Avoids the per-row N+1 of [`resolve_account_ref`] when a
/// listing renders several rows that each name an account.
///
/// Distinct ids are loaded once; an id with no live account is simply absent from
/// the map, leaving the missing-account decision to the caller (mirroring
/// [`resolve_account_ref`], which treats it as a server-side inconsistency).
///
/// # Errors
///
/// A database error if the account lookup fails.
pub async fn resolve_account_refs(
    conn: &mut PgConn,
    account_ids: &[Uuid],
) -> Result<HashMap<Uuid, AccountRef>> {
    let accounts = conn.find_accounts_by_ids(account_ids).await?;
    Ok(accounts
        .into_iter()
        .map(|account| {
            (
                account.id,
                AccountRef::new(
                    account.id,
                    account.username,
                    account.display_name,
                    account.avatar_url,
                ),
            )
        })
        .collect())
}

/// Builds the list of user-specific inputs a password is checked against for
/// strength, so a password cannot simply echo the account's own identifiers.
pub fn build_password_user_inputs<'a>(
    username: &'a str,
    display_name: Option<&'a str>,
    email_address: &'a str,
) -> Vec<&'a str> {
    let mut inputs = vec![username];
    inputs.extend(display_name);
    inputs.extend(email_address.split('@'));
    inputs
}
