//! Activity type enumeration for workspace audit logging.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoStaticStr};

/// Defines the type of activity performed in a workspace for audit logging.
///
/// This enumeration corresponds to the `ACTIVITY_TYPE` PostgreSQL enum and is used
/// to categorize different types of activities that occur within workspaces for comprehensive
/// audit trail and activity tracking.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(DbEnum, Display, EnumIter, EnumString, IntoStaticStr)]
#[ExistingTypePath = "crate::schema::sql_types::ActivityType"]
pub enum ActivityType {
    // Workspace activities
    /// Workspace was created
    #[db_rename = "workspace.created"]
    #[serde(rename = "workspace.created")]
    #[strum(serialize = "workspace.created")]
    WorkspaceCreated,

    /// Workspace settings or metadata were updated
    #[db_rename = "workspace.updated"]
    #[serde(rename = "workspace.updated")]
    #[strum(serialize = "workspace.updated")]
    WorkspaceUpdated,

    /// Workspace was deleted
    #[db_rename = "workspace.deleted"]
    #[serde(rename = "workspace.deleted")]
    #[strum(serialize = "workspace.deleted")]
    WorkspaceDeleted,

    // Member activities
    /// Member joined the workspace
    #[db_rename = "member.added"]
    #[serde(rename = "member.added")]
    #[strum(serialize = "member.added")]
    MemberAdded,

    /// Member information or preferences were updated
    #[db_rename = "member.updated"]
    #[serde(rename = "member.updated")]
    #[strum(serialize = "member.updated")]
    MemberUpdated,

    /// Member was removed from the workspace
    #[db_rename = "member.deleted"]
    #[serde(rename = "member.deleted")]
    #[strum(serialize = "member.deleted")]
    MemberDeleted,

    // Invite activities
    /// Invite was created
    #[db_rename = "invite.created"]
    #[serde(rename = "invite.created")]
    #[strum(serialize = "invite.created")]
    InviteCreated,

    /// Invite was accepted
    #[db_rename = "invite.accepted"]
    #[serde(rename = "invite.accepted")]
    #[strum(serialize = "invite.accepted")]
    InviteAccepted,

    /// Invite was declined
    #[db_rename = "invite.declined"]
    #[serde(rename = "invite.declined")]
    #[strum(serialize = "invite.declined")]
    InviteDeclined,

    /// Invite was canceled
    #[db_rename = "invite.canceled"]
    #[serde(rename = "invite.canceled")]
    #[strum(serialize = "invite.canceled")]
    InviteCanceled,

    // Connection activities
    /// Connection was created
    #[db_rename = "connection.created"]
    #[serde(rename = "connection.created")]
    #[strum(serialize = "connection.created")]
    ConnectionCreated,

    /// Connection was updated
    #[db_rename = "connection.updated"]
    #[serde(rename = "connection.updated")]
    #[strum(serialize = "connection.updated")]
    ConnectionUpdated,

    /// Connection was deleted
    #[db_rename = "connection.deleted"]
    #[serde(rename = "connection.deleted")]
    #[strum(serialize = "connection.deleted")]
    ConnectionDeleted,

    /// Connection started synchronization
    #[db_rename = "connection.sync.started"]
    #[serde(rename = "connection.sync.started")]
    #[strum(serialize = "connection.sync.started")]
    ConnectionSyncStarted,

    /// Connection completed synchronization
    #[db_rename = "connection.sync.completed"]
    #[serde(rename = "connection.sync.completed")]
    #[strum(serialize = "connection.sync.completed")]
    ConnectionSyncCompleted,

    /// Connection synchronization failed
    #[db_rename = "connection.sync.failed"]
    #[serde(rename = "connection.sync.failed")]
    #[strum(serialize = "connection.sync.failed")]
    ConnectionSyncFailed,

    // Webhook activities
    /// Webhook was created
    #[db_rename = "webhook.created"]
    #[serde(rename = "webhook.created")]
    #[strum(serialize = "webhook.created")]
    WebhookCreated,

    /// Webhook was updated
    #[db_rename = "webhook.updated"]
    #[serde(rename = "webhook.updated")]
    #[strum(serialize = "webhook.updated")]
    WebhookUpdated,

    /// Webhook was deleted
    #[db_rename = "webhook.deleted"]
    #[serde(rename = "webhook.deleted")]
    #[strum(serialize = "webhook.deleted")]
    WebhookDeleted,

    // File activities
    /// File was created
    #[db_rename = "file.created"]
    #[serde(rename = "file.created")]
    #[strum(serialize = "file.created")]
    FileCreated,

    /// File was updated
    #[db_rename = "file.updated"]
    #[serde(rename = "file.updated")]
    #[strum(serialize = "file.updated")]
    FileUpdated,

    /// File was deleted
    #[db_rename = "file.deleted"]
    #[serde(rename = "file.deleted")]
    #[strum(serialize = "file.deleted")]
    FileDeleted,

    // Pipeline activities
    /// Pipeline was created
    #[db_rename = "pipeline.created"]
    #[serde(rename = "pipeline.created")]
    #[strum(serialize = "pipeline.created")]
    PipelineCreated,

    /// Pipeline was updated
    #[db_rename = "pipeline.updated"]
    #[serde(rename = "pipeline.updated")]
    #[strum(serialize = "pipeline.updated")]
    PipelineUpdated,

    /// Pipeline was deleted
    #[db_rename = "pipeline.deleted"]
    #[serde(rename = "pipeline.deleted")]
    #[strum(serialize = "pipeline.deleted")]
    PipelineDeleted,

    /// Pipeline run was started
    #[db_rename = "pipeline.run.started"]
    #[serde(rename = "pipeline.run.started")]
    #[strum(serialize = "pipeline.run.started")]
    PipelineRunStarted,

    /// Pipeline run finished detection
    #[db_rename = "pipeline.run.analyzed"]
    #[serde(rename = "pipeline.run.analyzed")]
    #[strum(serialize = "pipeline.run.analyzed")]
    PipelineRunAnalyzed,

    /// Pipeline run completed
    #[db_rename = "pipeline.run.completed"]
    #[serde(rename = "pipeline.run.completed")]
    #[strum(serialize = "pipeline.run.completed")]
    PipelineRunCompleted,

    /// Pipeline run failed
    #[db_rename = "pipeline.run.failed"]
    #[serde(rename = "pipeline.run.failed")]
    #[strum(serialize = "pipeline.run.failed")]
    PipelineRunFailed,

    // Policy activities
    /// Policy was created
    #[db_rename = "policy.created"]
    #[serde(rename = "policy.created")]
    #[strum(serialize = "policy.created")]
    PolicyCreated,

    /// Policy was updated
    #[db_rename = "policy.updated"]
    #[serde(rename = "policy.updated")]
    #[strum(serialize = "policy.updated")]
    PolicyUpdated,

    /// Policy was deleted
    #[db_rename = "policy.deleted"]
    #[serde(rename = "policy.deleted")]
    #[strum(serialize = "policy.deleted")]
    PolicyDeleted,
}

impl ActivityType {
    /// The canonical dotted tag for this type, e.g. `file.created` or
    /// `pipeline.run.completed` — the same string used on the wire and in the DB,
    /// from the variant's `strum(serialize)`.
    pub fn as_tag(self) -> &'static str {
        self.into()
    }

    /// The object half of the tag: everything before the final segment, e.g.
    /// `file` for `file.created`, `pipeline.run` for `pipeline.run.completed`,
    /// `connection.sync` for `connection.sync.failed`.
    pub fn object_type(self) -> &'static str {
        let tag = self.as_tag();
        match tag.rsplit_once('.') {
            Some((object, _action)) => object,
            None => tag,
        }
    }

    /// The action half of the tag: the final segment, e.g. `created` for
    /// `file.created`, `completed` for `pipeline.run.completed`.
    pub fn action_type(self) -> &'static str {
        let tag = self.as_tag();
        match tag.rsplit_once('.') {
            Some((_object, action)) => action,
            None => tag,
        }
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn as_tag_matches_the_serde_tag_for_every_variant() {
        // Guards against the hand-written `as_tag` drifting from the serde rename
        // that defines the wire/DB form.
        for ty in ActivityType::iter() {
            let serde_tag = serde_json::to_value(ty).unwrap();
            assert_eq!(serde_tag.as_str().unwrap(), ty.as_tag(), "{ty:?}");
        }
    }

    #[test]
    fn object_and_action_recompose_to_the_tag_for_every_variant() {
        for ty in ActivityType::iter() {
            assert_eq!(
                format!("{}.{}", ty.object_type(), ty.action_type()),
                ty.as_tag(),
                "{ty:?}",
            );
        }
    }

    #[test]
    fn split_takes_the_last_segment_as_the_action() {
        // Two-part tag.
        assert_eq!(ActivityType::FileCreated.object_type(), "file");
        assert_eq!(ActivityType::FileCreated.action_type(), "created");
        // Three-part tags: object is everything before the final segment.
        assert_eq!(
            ActivityType::PipelineRunCompleted.object_type(),
            "pipeline.run"
        );
        assert_eq!(
            ActivityType::PipelineRunCompleted.action_type(),
            "completed"
        );
        assert_eq!(
            ActivityType::ConnectionSyncFailed.object_type(),
            "connection.sync"
        );
        assert_eq!(ActivityType::ConnectionSyncFailed.action_type(), "failed");
    }
}
