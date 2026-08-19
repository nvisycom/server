//! Response types for HTTP handlers.

use nvisy_postgres::types::CursorPage;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod account_ref;
mod accounts;
mod activities;
mod authentications;
mod catalog;
mod chat;
mod connection_syncs;
mod connections;
mod errors;
mod files;
mod invites;
mod members;
mod monitors;
mod notifications;
mod pipeline_runs;
mod pipelines;
mod policies;
mod tokens;
mod webhooks;
mod workspaces;

pub use account_ref::*;
pub use accounts::*;
pub use activities::*;
pub use authentications::*;
pub use catalog::*;
pub use chat::*;
pub use connection_syncs::*;
pub use connections::*;
pub use errors::*;
pub use files::*;
pub use invites::*;
pub use members::*;
pub use monitors::*;
pub use notifications::*;
pub use pipeline_runs::*;
pub use pipelines::*;
pub use policies::*;
pub use tokens::*;
pub use webhooks::*;
pub use workspaces::*;

/// Generic paginated response wrapper.
///
/// Provides a consistent structure for all paginated API responses with
/// cursor-based pagination support. When `next_cursor` is present, there
/// are more items to fetch.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "{T}Page")]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    /// Items in this page.
    pub items: Vec<T>,
    /// Total count of items matching the query (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
    /// Cursor to fetch the next page. Present only when more items exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl<T> Page<T> {
    /// Creates an empty page with no items.
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total: Some(0),
            next_cursor: None,
        }
    }

    /// Creates a new page from items and pagination metadata.
    pub fn new(items: Vec<T>, total: Option<i64>, next_cursor: Option<String>) -> Self {
        Self {
            items,
            total,
            next_cursor,
        }
    }

    /// Returns true if there are more items to fetch.
    pub fn has_more(&self) -> bool {
        self.next_cursor.is_some()
    }

    /// Maps items from one type to another.
    pub fn map<U, F>(self, f: F) -> Page<U>
    where
        F: FnMut(T) -> U,
    {
        Page {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            next_cursor: self.next_cursor,
        }
    }

    /// Creates a page from a cursor page, mapping items using the provided function.
    pub fn from_cursor_page<M, F>(page: CursorPage<M>, f: F) -> Self
    where
        F: FnMut(M) -> T,
    {
        Self {
            items: page.items.into_iter().map(f).collect(),
            total: page.total,
            next_cursor: page.next_cursor,
        }
    }

    /// Creates a page from a cursor page, mapping items with a fallible function.
    ///
    /// Returns the first error encountered while mapping (e.g. a decryption
    /// failure), otherwise the fully mapped page.
    pub fn try_from_cursor_page<M, F, E>(page: CursorPage<M>, f: F) -> Result<Self, E>
    where
        F: FnMut(M) -> Result<T, E>,
    {
        Ok(Self {
            items: page.items.into_iter().map(f).collect::<Result<_, _>>()?,
            total: page.total,
            next_cursor: page.next_cursor,
        })
    }

    /// Creates a page from a cursor page, mapping items and dropping any that map
    /// to `None` (e.g. a stored row whose shape no longer decodes).
    ///
    /// The cursor and total reflect the source page; only the rendered items are
    /// filtered, so pagination still advances past a skipped row.
    pub fn filter_from_cursor_page<M, F>(page: CursorPage<M>, f: F) -> Self
    where
        F: FnMut(M) -> Option<T>,
    {
        Self {
            items: page.items.into_iter().filter_map(f).collect(),
            total: page.total,
            next_cursor: page.next_cursor,
        }
    }
}
