//! Shared account-handle resolution for handler responses.
//!
//! Responses expose the creating/triggering account by its public username
//! rather than its internal id, so handlers resolve the handle from the account
//! id they already hold.

use nvisy_postgres::PgConn;
use nvisy_postgres::query::AccountRepository;
use nvisy_postgres::types::Handle;
use uuid::Uuid;

use crate::handler::response::AccountRef;
use crate::handler::{ErrorKind, Result};

/// The outcome of resolving an actor filter (a username supplied by the client)
/// against the accounts table.
///
/// The three states are distinct because a username that matches no account must
/// not be conflated with "no filter": an unknown actor should return an empty
/// listing, not every actor's activities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorFilter {
    /// No actor was requested; do not constrain by actor.
    Any,
    /// The username resolved to this account; constrain the listing to it.
    Account(Uuid),
    /// A username was requested but matches no account; the listing is empty.
    Unknown,
}

/// Resolves an optional actor `username` into an [`ActorFilter`].
///
/// `None` means the client asked for no actor filter ([`ActorFilter::Any`]); a
/// username that matches no account yields [`ActorFilter::Unknown`] so the caller
/// can return an empty result rather than an unfiltered one.
pub async fn resolve_actor(conn: &mut PgConn, username: Option<&Handle>) -> Result<ActorFilter> {
    let Some(username) = username else {
        return Ok(ActorFilter::Any);
    };
    Ok(match conn.find_account_by_username(username).await? {
        Some(account) => ActorFilter::Account(account.id),
        None => ActorFilter::Unknown,
    })
}

/// Resolves a public reference to a required account (e.g. a resource's
/// creator, or whoever triggered an action).
///
/// The account is expected to exist — typically the authenticated caller — so a
/// missing row is a server-side inconsistency rather than a client error.
pub async fn resolve_account_ref(conn: &mut PgConn, account_id: Uuid) -> Result<AccountRef> {
    conn.find_account_by_id(account_id)
        .await?
        .map(|account| AccountRef::new(account.username, account.display_name, account.avatar_url))
        .ok_or_else(|| ErrorKind::InternalServerError.with_message("account not found"))
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
