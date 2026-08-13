//! Sync mode enumeration indicating the direction a connection syncs.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// The direction a connection syncs data.
///
/// Corresponds to the `SYNC_MODE` PostgreSQL enum: `Import` fetches objects from
/// the connection into the workspace; `Export` pushes workspace files out.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::SyncMode"]
pub enum SyncMode {
    /// Fetch objects from the connection into the workspace.
    #[db_rename = "import"]
    #[serde(rename = "import")]
    #[default]
    Import,

    /// Push workspace files out to the connection.
    #[db_rename = "export"]
    #[serde(rename = "export")]
    Export,
}

impl SyncMode {
    /// Returns whether the connection imports data into the workspace.
    #[inline]
    pub fn is_import(self) -> bool {
        matches!(self, SyncMode::Import)
    }

    /// Returns whether the connection exports data out of the workspace.
    #[inline]
    pub fn is_export(self) -> bool {
        matches!(self, SyncMode::Export)
    }
}
