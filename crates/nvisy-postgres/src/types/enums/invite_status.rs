//! Invite status enumeration for workspace invitation tracking.

use super::db_enum;

db_enum! {
    /// The current status of a workspace invitation.
    ///
    /// Corresponds to the `INVITE_STATUS` PostgreSQL enum and tracks the lifecycle
    /// of an invitation from creation to resolution.
    pub enum InviteStatus: Default = Pending, "crate::schema::sql_types::InviteStatus" {
        /// Sent and awaiting a response from the invitee.
        Pending = "pending",
        /// Accepted; the member has been added to the workspace.
        Accepted = "accepted",
        /// Declined by the invitee.
        Declined = "declined",
        /// Canceled by the person who sent it.
        Canceled = "canceled",
        /// Expired due to timeout (automatic system action).
        Expired = "expired",
        /// Revoked by a workspace administrator.
        Revoked = "revoked",
    }
}
