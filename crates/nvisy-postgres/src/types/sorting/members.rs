//! Sorting options for workspace member queries.

use serde::{Deserialize, Serialize};

use super::SortBy;

/// Fields available for sorting workspace members.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MemberSortField {
    /// Sort by display name.
    Name,
    /// Sort by join date.
    #[default]
    Date,
}

/// Sorting specification for workspace members.
pub type MemberSortBy = SortBy<MemberSortField>;
