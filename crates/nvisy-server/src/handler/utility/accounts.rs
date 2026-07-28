//! Shared account-handle resolution for handler responses.
//!
//! Responses expose the creating/triggering account by its public username
//! rather than its internal id, so handlers resolve the handle from the account
//! id they already hold.

use nvisy_postgres::PgConn;
use nvisy_postgres::query::AccountRepository;
use nvisy_postgres::types::Username;
use uuid::Uuid;

use crate::handler::{ErrorKind, Result};

/// Resolves the handle of a required account (e.g. a resource's creator).
///
/// The account is expected to exist — typically the authenticated caller — so a
/// missing row is a server-side inconsistency rather than a client error.
pub async fn resolve_creator_username(conn: &mut PgConn, account_id: Uuid) -> Result<Username> {
    conn.find_account_by_id(account_id)
        .await?
        .map(|account| account.username)
        .ok_or_else(|| ErrorKind::InternalServerError.with_message("Creator account not found"))
}

/// Resolves the handle of an optional account (e.g. whoever triggered a run).
///
/// Returns `None` when no account is recorded; a recorded-but-missing account
/// also resolves to `None` rather than failing the request.
pub async fn resolve_trigger_username(
    conn: &mut PgConn,
    account_id: Option<Uuid>,
) -> Result<Option<Username>> {
    let Some(account_id) = account_id else {
        return Ok(None);
    };
    Ok(conn
        .find_account_by_id(account_id)
        .await?
        .map(|account| account.username))
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
