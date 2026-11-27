//! Pairing a row with a public reference to its associated account.

use diesel::{Queryable, Selectable};
use uuid::Uuid;

use crate::schema::accounts;
use crate::types::Handle;

/// A public reference to an account, joined alongside a resource.
///
/// Selectable from the `accounts` table, so a joined lookup selects
/// `(Model::as_select(), AccountRefRow::as_select())` (or `.nullable()` on a left
/// join) and receives a named `AccountRefRow` rather than a positional tuple.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable)]
#[diesel(table_name = accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountRefRow {
    /// Immutable id of the account.
    pub id: Uuid,
    /// Handle of the account (display).
    pub username: Handle,
    /// Human-readable display name, when set.
    pub display_name: Option<String>,
    /// Serve path of the account's avatar, when set.
    pub avatar_url: Option<String>,
}

/// A resource row paired with a reference to its associated account.
///
/// Returned by repository lookups that join the account, so callers get a named
/// `item` and `account` rather than a positional tuple. `A` is
/// `Option<AccountRefRow>` where the account is left-joined and may have been
/// removed (a review event's actor).
#[derive(Debug, Clone, PartialEq)]
pub struct WithAccountRef<T, A = AccountRefRow> {
    /// The resource row.
    pub item: T,
    /// The associated account.
    pub account: A,
}

impl<T, A> From<(T, A)> for WithAccountRef<T, A> {
    fn from((item, account): (T, A)) -> Self {
        Self { item, account }
    }
}
