//! Sorting options for workspace invite queries.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::SortBy;

/// Fields available for sorting workspace invites.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum InviteSortField {
    /// Sort by invitee email.
    Email,
    /// Sort by creation date.
    #[default]
    Date,
}

/// Sorting specification for workspace invites.
pub type InviteSortBy = SortBy<InviteSortField>;
