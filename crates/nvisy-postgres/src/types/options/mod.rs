//! Sorting and filtering options for database queries.

mod files;
mod integrations;
mod invites;
mod members;

pub use files::{FileFilter, FileFormat, FileSortBy};
pub use integrations::IntegrationFilter;
pub use invites::{InviteFilter, InviteSortBy};
pub use members::{MemberFilter, MemberSortBy};
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
