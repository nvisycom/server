//! Studio session status enumeration for LLM-assisted editing sessions.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the lifecycle status of a studio editing session.
///
/// This enumeration corresponds to the `STUDIO_SESSION_STATUS` PostgreSQL enum and is used
/// to track the state of LLM-assisted document editing sessions as they progress through
/// their lifecycle from active use to archival.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::StudioSessionStatus"]
pub enum StudioSessionStatus {
    /// Session is currently active and in use
    #[db_rename = "active"]
    #[serde(rename = "active")]
    #[default]
    Active,

    /// Session is temporarily paused but can be resumed
    #[db_rename = "paused"]
    #[serde(rename = "paused")]
    Paused,

    /// Session has been archived and is no longer active
    #[db_rename = "archived"]
    #[serde(rename = "archived")]
    Archived,
}

impl StudioSessionStatus {
    /// Returns whether the session is currently active.
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(self, StudioSessionStatus::Active)
    }

    /// Returns whether the session is paused.
    #[inline]
    pub fn is_paused(self) -> bool {
        matches!(self, StudioSessionStatus::Paused)
    }

    /// Returns whether the session is archived.
    #[inline]
    pub fn is_archived(self) -> bool {
        matches!(self, StudioSessionStatus::Archived)
    }

    /// Returns whether the session can accept new messages or tool calls.
    #[inline]
    pub fn can_accept_input(self) -> bool {
        matches!(self, StudioSessionStatus::Active)
    }

    /// Returns whether the session can be resumed.
    #[inline]
    pub fn can_resume(self) -> bool {
        matches!(self, StudioSessionStatus::Paused)
    }

    /// Returns whether the session can be paused.
    #[inline]
    pub fn can_pause(self) -> bool {
        matches!(self, StudioSessionStatus::Active)
    }

    /// Returns whether the session can be archived.
    #[inline]
    pub fn can_archive(self) -> bool {
        matches!(
            self,
            StudioSessionStatus::Active | StudioSessionStatus::Paused
        )
    }

    /// Returns whether the session is in a final state.
    #[inline]
    pub fn is_final(self) -> bool {
        matches!(self, StudioSessionStatus::Archived)
    }

    /// Returns session statuses that are considered active (not archived).
    pub fn active_statuses() -> &'static [StudioSessionStatus] {
        &[StudioSessionStatus::Active, StudioSessionStatus::Paused]
    }
}
