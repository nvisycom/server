//! Pairing a row with a public reference to its associated account.

use diesel::Queryable;

use crate::types::Handle;

/// A public reference to an account, joined alongside a resource.
///
/// Loads directly from a `(username, display_name, avatar_url)` column group, so
/// a joined lookup selects `(Model::as_select(), AccountRefRow::columns())` and
/// receives a named `AccountRefRow` rather than a positional tuple.
#[derive(Debug, Clone, PartialEq, Queryable)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AccountRefRow {
    /// Handle of the account.
    pub username: Handle,
    /// Human-readable display name, when set.
    pub display_name: Option<String>,
    /// Serve path of the account's avatar, when set.
    pub avatar_url: Option<String>,
}

/// A resource row paired with a reference to its associated account.
///
/// Returned by repository lookups that join the account, so callers get a named
/// `item` and `account` rather than a positional tuple.
#[derive(Debug, Clone, PartialEq)]
pub struct WithAccountRef<T> {
    /// The resource row.
    pub item: T,
    /// The associated account.
    pub account: AccountRefRow,
}
