//! Studio tool status enumeration for tool execution tracking.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the execution status of a studio tool call.
///
/// This enumeration corresponds to the `STUDIO_TOOL_STATUS` PostgreSQL enum and is used
/// to track the state of tool invocations within studio sessions as they progress
/// from pending through execution to completion or cancellation.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::StudioToolStatus"]
pub enum StudioToolStatus {
    /// Tool call is queued and waiting to be executed
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// Tool is currently being executed
    #[db_rename = "running"]
    #[serde(rename = "running")]
    Running,

    /// Tool execution completed successfully
    #[db_rename = "completed"]
    #[serde(rename = "completed")]
    Completed,

    /// Tool execution was cancelled
    #[db_rename = "cancelled"]
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl StudioToolStatus {
    /// Returns whether the tool call is pending execution.
    #[inline]
    pub fn is_pending(self) -> bool {
        matches!(self, StudioToolStatus::Pending)
    }

    /// Returns whether the tool is currently running.
    #[inline]
    pub fn is_running(self) -> bool {
        matches!(self, StudioToolStatus::Running)
    }

    /// Returns whether the tool execution completed successfully.
    #[inline]
    pub fn is_completed(self) -> bool {
        matches!(self, StudioToolStatus::Completed)
    }

    /// Returns whether the tool execution was cancelled.
    #[inline]
    pub fn is_cancelled(self) -> bool {
        matches!(self, StudioToolStatus::Cancelled)
    }

    /// Returns whether the tool is in a final state.
    #[inline]
    pub fn is_final(self) -> bool {
        matches!(
            self,
            StudioToolStatus::Completed | StudioToolStatus::Cancelled
        )
    }

    /// Returns whether the tool can be started.
    #[inline]
    pub fn can_start(self) -> bool {
        matches!(self, StudioToolStatus::Pending)
    }

    /// Returns whether the tool can be cancelled.
    #[inline]
    pub fn can_cancel(self) -> bool {
        matches!(self, StudioToolStatus::Pending | StudioToolStatus::Running)
    }

    /// Returns whether the tool execution is active (not final).
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(self, StudioToolStatus::Pending | StudioToolStatus::Running)
    }

    /// Returns tool statuses that are considered active (not final).
    pub fn active_statuses() -> &'static [StudioToolStatus] {
        &[StudioToolStatus::Pending, StudioToolStatus::Running]
    }

    /// Returns tool statuses that represent final states.
    pub fn final_statuses() -> &'static [StudioToolStatus] {
        &[StudioToolStatus::Completed, StudioToolStatus::Cancelled]
    }
}
