//! Pipeline trigger type enumeration indicating how a pipeline run was initiated.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines how a pipeline run was initiated.
///
/// This enumeration corresponds to the `PIPELINE_TRIGGER_TYPE` PostgreSQL enum and is used
/// to track whether a run was manually triggered, triggered by a source connector, or scheduled.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::PipelineTriggerType"]
pub enum PipelineTriggerType {
    /// Manually triggered by user
    #[db_rename = "manual"]
    #[serde(rename = "manual")]
    #[default]
    Manual,

    /// Triggered by source connector (upload, webhook, etc.)
    #[db_rename = "source"]
    #[serde(rename = "source")]
    Source,

    /// Triggered by schedule
    #[db_rename = "scheduled"]
    #[serde(rename = "scheduled")]
    Scheduled,
}

impl PipelineTriggerType {
    /// Returns whether the run was manually triggered.
    #[inline]
    pub fn is_manual(self) -> bool {
        matches!(self, PipelineTriggerType::Manual)
    }

    /// Returns whether the run was triggered by a source connector.
    #[inline]
    pub fn is_source(self) -> bool {
        matches!(self, PipelineTriggerType::Source)
    }

    /// Returns whether the run was scheduled.
    #[inline]
    pub fn is_scheduled(self) -> bool {
        matches!(self, PipelineTriggerType::Scheduled)
    }

    /// Returns whether the run was triggered automatically (source or scheduled).
    #[inline]
    pub fn is_automatic(self) -> bool {
        matches!(
            self,
            PipelineTriggerType::Source | PipelineTriggerType::Scheduled
        )
    }

    /// Returns whether the run was triggered by user action.
    #[inline]
    pub fn is_user_initiated(self) -> bool {
        matches!(self, PipelineTriggerType::Manual)
    }
}
