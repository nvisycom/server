//! Pairing a row with a public reference to its associated account.

use crate::types::Handle;

/// A public reference to an account, joined alongside a resource.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountRefRow {
    /// Handle of the account.
    pub username: Handle,
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
