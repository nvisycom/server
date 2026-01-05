//! Invite status enumeration for workspace invitation tracking.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the current status of a workspace invitation.
///
/// This enumeration corresponds to the `INVITE_STATUS` PostgreSQL enum and is used
/// to track the lifecycle of workspace invitations from creation to resolution.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::InviteStatus"]
pub enum InviteStatus {
    /// Invitation has been sent and is awaiting a response from the invitee
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// Invitation has been accepted and the member has been added to the workspace
    #[db_rename = "accepted"]
    #[serde(rename = "accepted")]
    Accepted,

    /// Invitation was declined by the invitee
    #[db_rename = "declined"]
    #[serde(rename = "declined")]
    Declined,

    /// Invitation was canceled by the person who sent it
    #[db_rename = "canceled"]
    #[serde(rename = "canceled")]
    Canceled,

    /// Invitation expired due to timeout (automatic system action)
    #[db_rename = "expired"]
    #[serde(rename = "expired")]
    Expired,

    /// Invitation was revoked by a workspace administrator
    #[db_rename = "revoked"]
    #[serde(rename = "revoked")]
    Revoked,
}

impl InviteStatus {
    /// Returns whether this invitation is still active and can be used.
    #[inline]
    pub fn is_active(self) -> bool {
        matches!(self, InviteStatus::Pending)
    }

    /// Returns whether this invitation has been resolved (accepted or declined by invitee).
    #[inline]
    pub fn is_resolved(self) -> bool {
        matches!(self, InviteStatus::Accepted | InviteStatus::Declined)
    }

    /// Returns whether this invitation was terminated by the system or administrators.
    #[inline]
    pub fn is_terminated(self) -> bool {
        matches!(
            self,
            InviteStatus::Canceled | InviteStatus::Expired | InviteStatus::Revoked
        )
    }
}
