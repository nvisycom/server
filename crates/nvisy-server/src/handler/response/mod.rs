//! Response types for HTTP handlers.

use nvisy_postgres::types::CursorPage;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod account_api_tokens;
mod account_identities;
mod account_notifications;
mod account_ref;
mod accounts;
mod analytics;
mod authentications;
mod capabilities;
mod monitors;
mod workspace_activities;
mod workspace_assignments;
mod workspace_connection_syncs;
mod workspace_connections;
mod workspace_detections;
mod workspace_files;
mod workspace_invites;
mod workspace_members;
mod workspace_pipelines;
mod workspace_policies;
mod workspace_providers;
mod workspace_redactions;
mod workspace_thread_anchors;
mod workspace_thread_comments;
mod workspace_threads;
mod workspace_webhooks;
mod workspaces;

pub use account_api_tokens::*;
pub use account_identities::*;
pub use account_notifications::*;
pub use account_ref::*;
pub use accounts::*;
pub use analytics::*;
pub use authentications::*;
pub use capabilities::*;
pub use monitors::*;
pub use workspace_activities::*;
pub use workspace_assignments::*;
pub use workspace_connection_syncs::*;
pub use workspace_connections::*;
pub use workspace_detections::*;
pub use workspace_files::*;
pub use workspace_invites::*;
pub use workspace_members::*;
pub use workspace_pipelines::*;
pub use workspace_policies::*;
pub use workspace_providers::*;
pub use workspace_redactions::*;
pub use workspace_thread_anchors::*;
pub use workspace_thread_comments::*;
pub use workspace_threads::*;
pub use workspace_webhooks::*;
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
