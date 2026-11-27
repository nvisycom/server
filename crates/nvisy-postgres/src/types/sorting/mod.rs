//! Sorting options for database queries.

mod invites;
mod members;

pub use invites::{InviteSortBy, InviteSortField};
pub use members::{MemberSortBy, MemberSortField};
use serde::{Deserialize, Serialize};

/// Sort order direction.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Ascending order (A-Z, oldest first, smallest first).
    Ascending,
    /// Descending order (Z-A, newest first, largest first).
    #[default]
    Descending,
}

/// Generic sort specification with field and order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename = "Sort{F}"))]
pub struct SortBy<F> {
    /// The field to sort by.
    pub field: F,
    /// The sort order direction.
    #[serde(default)]
    pub order: Direction,
}

impl<F: Default> Default for SortBy<F> {
    fn default() -> Self {
        Self {
            field: F::default(),
            order: Direction::default(),
        }
    }
}

impl<F> SortBy<F> {
    /// Creates a new sort specification with the given field and order.
    #[inline]
    pub fn new(field: F, order: Direction) -> Self {
        Self { field, order }
    }

    /// Creates a new sort specification with ascending order.
    #[inline]
    pub fn asc(field: F) -> Self {
        Self {
            field,
            order: Direction::Ascending,
        }
    }

    /// Creates a new sort specification with descending order.
    #[inline]
    pub fn desc(field: F) -> Self {
        Self {
            field,
            order: Direction::Descending,
        }
    }
}
