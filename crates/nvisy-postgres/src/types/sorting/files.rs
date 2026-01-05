//! Sorting options for document file queries.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::SortBy;

/// Fields available for sorting document files.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileSortField {
    /// Sort by file name.
    Name,
    /// Sort by upload date.
    #[default]
    Date,
    /// Sort by file size.
    Size,
}

/// Sorting specification for document files.
pub type FileSortBy = SortBy<FileSortField>;
