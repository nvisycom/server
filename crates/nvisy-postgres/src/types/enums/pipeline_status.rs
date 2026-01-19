//! Pipeline status enumeration indicating the lifecycle state of a pipeline.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the lifecycle status of a pipeline definition.
///
/// This enumeration corresponds to the `PIPELINE_STATUS` PostgreSQL enum and is used
/// to track whether a pipeline is being configured, active and ready to run, or disabled.
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
    #[db_rename = "active"]
    #[serde(rename = "active")]
    Active,

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

    /// Returns whether the pipeline is active.
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(self, PipelineStatus::Active)
    }

    /// Returns whether the pipeline is disabled.
    #[inline]
    pub fn is_disabled(self) -> bool {
        matches!(self, PipelineStatus::Disabled)
    }

    /// Returns whether the pipeline can be executed.
    #[inline]
    pub fn is_runnable(self) -> bool {
        matches!(self, PipelineStatus::Active)
    }

    /// Returns whether the pipeline can be edited.
    #[inline]
    pub fn is_editable(self) -> bool {
        matches!(self, PipelineStatus::Draft | PipelineStatus::Disabled)
    }
}
