//! Invite status enumeration for project invitation tracking.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Defines the current status of a project invitation.
///
/// This enumeration corresponds to the `INVITE_STATUS` PostgreSQL enum and is used
/// to track the lifecycle of project invitations from creation to resolution.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[ExistingTypePath = "crate::schema::sql_types::InviteStatus"]
pub enum InviteStatus {
    /// Invitation has been sent and is awaiting a response from the invitee
    #[db_rename = "pending"]
    #[serde(rename = "pending")]
    #[default]
    Pending,

    /// Invitation has been accepted and the member has been added to the project
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

    /// Invitation was revoked by a project administrator
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

    /// Returns whether this invitation resulted in successful project membership.
    #[inline]
    pub fn is_successful(self) -> bool {
        matches!(self, InviteStatus::Accepted)
    }

    /// Returns whether this invitation was rejected or failed.
    #[inline]
    pub fn is_failed(self) -> bool {
        matches!(
            self,
            InviteStatus::Declined
                | InviteStatus::Canceled
                | InviteStatus::Expired
                | InviteStatus::Revoked
        )
    }

    /// Returns whether the invitation can be resent or recreated.
    #[inline]
    pub fn can_be_resent(self) -> bool {
        matches!(
            self,
            InviteStatus::Declined | InviteStatus::Expired | InviteStatus::Revoked
        )
    }
}
