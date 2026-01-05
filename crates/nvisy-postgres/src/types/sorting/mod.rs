//! Sorting options for database queries.

mod files;
mod invites;
mod members;

pub use files::{FileSortBy, FileSortField};
pub use invites::{InviteSortBy, InviteSortField};
pub use members::{MemberSortBy, MemberSortField};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Sort order direction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Ascending order (A-Z, oldest first, smallest first).
    Asc,
    /// Descending order (Z-A, newest first, largest first).
    #[default]
    Desc,
}

/// Generic sort specification with field and order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct SortBy<F> {
    /// The field to sort by.
    pub field: F,
    /// The sort order direction.
    #[serde(default)]
    pub order: SortOrder,
}

impl<F: Default> Default for SortBy<F> {
    fn default() -> Self {
        Self {
            field: F::default(),
            order: SortOrder::default(),
        }
    }
}

impl<F> SortBy<F> {
    /// Creates a new sort specification with the given field and order.
    #[inline]
    pub fn new(field: F, order: SortOrder) -> Self {
        Self { field, order }
    }

    /// Creates a new sort specification with ascending order.
    #[inline]
    pub fn asc(field: F) -> Self {
        Self {
            field,
            order: SortOrder::Asc,
        }
    }

    /// Creates a new sort specification with descending order.
    #[inline]
    pub fn desc(field: F) -> Self {
        Self {
            field,
            order: SortOrder::Desc,
        }
    }

    /// Returns whether the sort order is ascending.
    #[inline]
    pub fn is_asc(&self) -> bool {
        matches!(self.order, SortOrder::Asc)
    }

    /// Returns whether the sort order is descending.
    #[inline]
    pub fn is_desc(&self) -> bool {
        matches!(self.order, SortOrder::Desc)
    }
}
