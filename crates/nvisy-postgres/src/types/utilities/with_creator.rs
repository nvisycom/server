//! Pairing a row with the public identity of the account that created it.

use crate::types::Handle;

/// The public identity of a creator account, joined alongside a resource.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatorRow {
    /// Handle of the creator account.
    pub username: Handle,
    /// Serve path of the creator's avatar, when set.
    pub avatar_url: Option<String>,
}

/// A resource row paired with its creator's identity.
///
/// Returned by repository lookups that join the creating account, so callers get
/// a named `item` and `creator` rather than a positional tuple.
#[derive(Debug, Clone, PartialEq)]
pub struct WithCreator<T> {
    /// The resource row.
    pub item: T,
    /// The creator's identity.
    pub creator: CreatorRow,
}
