//! Run type enumeration for integration run classification.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the type of an integration run.
///
/// This enumeration corresponds to the `RUN_TYPE` PostgreSQL enum and is used
/// to classify how an integration run was triggered.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::RunType"]
pub enum RunType {
    /// Run triggered manually by a user
    #[db_rename = "manual"]
    #[serde(rename = "manual")]
    #[default]
    Manual,

    /// Run triggered by a schedule
    #[db_rename = "scheduled"]
    #[serde(rename = "scheduled")]
    Scheduled,

    /// Run triggered by an external event or webhook
    #[db_rename = "triggered"]
    #[serde(rename = "triggered")]
    Triggered,
}

impl RunType {
    /// Returns whether this is a manual run.
    #[inline]
    pub fn is_manual(self) -> bool {
        matches!(self, RunType::Manual)
    }

    /// Returns whether this is a scheduled run.
    #[inline]
    pub fn is_scheduled(self) -> bool {
        matches!(self, RunType::Scheduled)
    }

    /// Returns whether this is a triggered run.
    #[inline]
    pub fn is_triggered(self) -> bool {
        matches!(self, RunType::Triggered)
    }

    /// Returns whether this is an automated run (scheduled or triggered).
    #[inline]
    pub fn is_automated(self) -> bool {
        matches!(self, RunType::Scheduled | RunType::Triggered)
    }
}
