//! Pipeline status enumeration indicating the lifecycle state of a pipeline.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the lifecycle status of a pipeline definition.
///
/// This enumeration corresponds to the `PIPELINE_STATUS` PostgreSQL enum and is used
/// to track whether a pipeline is being configured, enabled and ready to run, or disabled.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::PipelineStatus"]
pub enum PipelineStatus {
    /// Pipeline is being configured
    #[db_rename = "draft"]
    #[serde(rename = "draft")]
    #[default]
    Draft,

    /// Pipeline is ready to run
    #[db_rename = "enabled"]
    #[serde(rename = "enabled")]
    Enabled,

    /// Pipeline is disabled
    #[db_rename = "disabled"]
    #[serde(rename = "disabled")]
    Disabled,
}

impl PipelineStatus {
    /// Returns whether the pipeline is in draft status.
    #[inline]
    pub fn is_draft(self) -> bool {
        matches!(self, PipelineStatus::Draft)
    }

    /// Returns whether the pipeline is enabled.
    #[inline]
    pub fn is_enabled(self) -> bool {
        matches!(self, PipelineStatus::Enabled)
    }

    /// Returns whether the pipeline is disabled.
    #[inline]
    pub fn is_disabled(self) -> bool {
        matches!(self, PipelineStatus::Disabled)
    }
}
