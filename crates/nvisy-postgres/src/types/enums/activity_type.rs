//! Activity type enumeration for workspace audit logging.

use super::db_enum;

db_enum! {
    /// The type of activity performed in a workspace, for audit logging.
    ///
    /// Corresponds to the `ACTIVITY_TYPE` PostgreSQL enum and categorizes the
    /// activities that occur within workspaces for a comprehensive audit trail.
    pub enum ActivityType = "crate::schema::sql_types::ActivityType" {
        /// Workspace was created.
        WorkspaceCreated = "workspace.created",
        /// Workspace settings or metadata were updated.
        WorkspaceUpdated = "workspace.updated",
        /// Workspace was deleted.
        WorkspaceDeleted = "workspace.deleted",
        /// Member was added to a workspace.
        MemberAdded = "member.added",
        /// Member's role or permissions were updated.
        MemberUpdated = "member.updated",
        /// Member was removed from a workspace.
        MemberDeleted = "member.deleted",
        /// Invitation was created.
        InviteCreated = "invite.created",
        /// Invitation was accepted.
        InviteAccepted = "invite.accepted",
        /// Invitation was declined.
        InviteDeclined = "invite.declined",
        /// Invitation was canceled.
        InviteCanceled = "invite.canceled",
        /// Connection was created.
        ConnectionCreated = "connection.created",
        /// Connection was updated.
        ConnectionUpdated = "connection.updated",
        /// Connection was deleted.
        ConnectionDeleted = "connection.deleted",
        /// Connection sync started.
        ConnectionSyncStarted = "connection.sync.started",
        /// Connection sync completed.
        ConnectionSyncCompleted = "connection.sync.completed",
        /// Connection sync failed.
        ConnectionSyncFailed = "connection.sync.failed",
        /// Provider was created.
        ProviderCreated = "provider.created",
        /// Provider was updated.
        ProviderUpdated = "provider.updated",
        /// Provider was deleted.
        ProviderDeleted = "provider.deleted",
        /// Webhook was created.
        WebhookCreated = "webhook.created",
        /// Webhook was updated.
        WebhookUpdated = "webhook.updated",
        /// Webhook was deleted.
        WebhookDeleted = "webhook.deleted",
        /// Document was created.
        DocumentCreated = "document.created",
        /// Document was updated.
        DocumentUpdated = "document.updated",
        /// Document was deleted.
        DocumentDeleted = "document.deleted",
        /// A document's review was verified.
        ReviewVerified = "review.verified",
        /// A document's review was assigned to a reviewer.
        ReviewAssigned = "review.assigned",
        /// A document's review assignee was cleared.
        ReviewUnassigned = "review.unassigned",
        /// Pipeline was created.
        PipelineCreated = "pipeline.created",
        /// Pipeline was updated.
        PipelineUpdated = "pipeline.updated",
        /// Pipeline was deleted.
        PipelineDeleted = "pipeline.deleted",
        /// Detection was started.
        DetectionStarted = "pipeline.detection.started",
        /// Detection finished analysis.
        DetectionCompleted = "pipeline.detection.completed",
        /// Detection failed.
        DetectionFailed = "pipeline.detection.failed",
        /// Redaction was created.
        RedactionCreated = "pipeline.redaction.created",
        /// Policy was created.
        PolicyCreated = "policy.created",
        /// Policy was updated.
        PolicyUpdated = "policy.updated",
        /// Policy was deleted.
        PolicyDeleted = "policy.deleted",
        /// A temporary policy was promoted to permanent.
        PolicyPromoted = "policy.promoted",
        /// A thread was opened.
        ThreadOpened = "thread.opened",
        /// A thread was closed.
        ThreadClosed = "thread.closed",
        /// A thread was reopened.
        ThreadReopened = "thread.reopened",
        /// A thread's title was changed.
        ThreadRenamed = "thread.renamed",
        /// A thread was deleted.
        ThreadDeleted = "thread.deleted",
        /// A comment (message) was posted in a thread.
        ThreadCommentCreated = "thread.comment.created",
    }
}

impl ActivityType {
    /// The canonical dotted tag for this type, e.g. `document.created` or
    /// `pipeline.redaction.created` — the same string used on the wire and in the
    /// DB, from the variant's `strum(serialize)`.
    pub fn as_tag(self) -> &'static str {
        self.into()
    }

    /// The object half of the tag: everything before the final segment, e.g.
    /// `document` for `document.created`, `pipeline.redaction` for
    /// `pipeline.redaction.created`, `connection.sync` for
    /// `connection.sync.failed`.
    pub fn object_type(self) -> &'static str {
        let tag = self.as_tag();
        match tag.rsplit_once('.') {
            Some((object, _action)) => object,
            None => tag,
        }
    }

    /// The action half of the tag: the final segment, e.g. `created` for
    /// `document.created`, `created` for `pipeline.redaction.created`.
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
        assert_eq!(ActivityType::DocumentCreated.object_type(), "document");
        assert_eq!(ActivityType::DocumentCreated.action_type(), "created");
        // Three-part tags: object is everything before the final segment.
        assert_eq!(
            ActivityType::RedactionCreated.object_type(),
            "pipeline.redaction"
        );
        assert_eq!(ActivityType::RedactionCreated.action_type(), "created");
        assert_eq!(
            ActivityType::ConnectionSyncFailed.object_type(),
            "connection.sync"
        );
        assert_eq!(ActivityType::ConnectionSyncFailed.action_type(), "failed");
    }
}
