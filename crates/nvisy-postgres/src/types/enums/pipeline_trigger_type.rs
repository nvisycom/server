//! Pipeline trigger type enumeration indicating how a pipeline run was initiated.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines how a pipeline run was initiated.
///
/// This enumeration corresponds to the `PIPELINE_TRIGGER_TYPE` PostgreSQL enum:
/// a run is either started directly by a user or automatically by the system
/// (for example, a file upload that the pipeline auto-redacts).
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::PipelineTriggerType"]
pub enum PipelineTriggerType {
    /// Started directly by a user.
    #[db_rename = "user"]
    #[serde(rename = "user")]
    #[default]
    User,

    /// Started automatically by the system (e.g. a file upload auto-redacted by
    /// the pipeline).
    #[db_rename = "system"]
    #[serde(rename = "system")]
    System,
}

impl PipelineTriggerType {
    /// Returns whether the run was started directly by a user.
    #[inline]
    pub fn is_user(self) -> bool {
        matches!(self, PipelineTriggerType::User)
    }

    /// Returns whether the run was started automatically by the system.
    #[inline]
    pub fn is_system(self) -> bool {
        matches!(self, PipelineTriggerType::System)
    }
}
