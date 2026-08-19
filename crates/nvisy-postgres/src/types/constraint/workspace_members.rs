//! Workspace members table constraint violations.

use strum::EnumString;

/// Workspace members table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumString)]
pub enum WorkspaceMemberConstraints {
    #[strum(serialize = "workspace_members_pkey")]
    MembershipUnique,
}
