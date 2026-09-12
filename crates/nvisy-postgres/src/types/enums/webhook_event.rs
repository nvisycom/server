//! Webhook event type enumeration for webhook event subscriptions.

use super::db_enum;

db_enum! {
    /// The types of events that can trigger webhook delivery.
    ///
    /// Corresponds to the `WEBHOOK_EVENT` PostgreSQL enum and configures which
    /// events a webhook receives.
    pub enum WebhookEvent = "crate::schema::sql_types::WebhookEvent" {
        /// A new document was created.
        DocumentCreated = "document.created",
        /// A document was updated.
        DocumentUpdated = "document.updated",
        /// A document was deleted.
        DocumentDeleted = "document.deleted",
        /// A member was added.
        MemberAdded = "member.added",
        /// A member was removed.
        MemberDeleted = "member.deleted",
        /// A member's role or permissions were updated.
        MemberUpdated = "member.updated",
        /// A connection was created.
        ConnectionCreated = "connection.created",
        /// A connection was updated.
        ConnectionUpdated = "connection.updated",
        /// A connection was deleted.
        ConnectionDeleted = "connection.deleted",
        /// A connection sync started.
        ConnectionSyncStarted = "connection.sync.started",
        /// A connection sync completed.
        ConnectionSyncCompleted = "connection.sync.completed",
        /// A connection sync failed.
        ConnectionSyncFailed = "connection.sync.failed",
        /// A provider was created.
        ProviderCreated = "provider.created",
        /// A provider was updated.
        ProviderUpdated = "provider.updated",
        /// A provider was deleted.
        ProviderDeleted = "provider.deleted",
        /// A document's review was verified.
        ReviewVerified = "review.verified",
        /// A document's review was assigned to a reviewer.
        ReviewAssigned = "review.assigned",
        /// A document's review assignee was cleared.
        ReviewUnassigned = "review.unassigned",
        /// A pipeline was created.
        PipelineCreated = "pipeline.created",
        /// A pipeline was updated.
        PipelineUpdated = "pipeline.updated",
        /// A pipeline was deleted.
        PipelineDeleted = "pipeline.deleted",
        /// A detection was started.
        DetectionStarted = "pipeline.detection.started",
        /// A detection finished analysis.
        DetectionCompleted = "pipeline.detection.completed",
        /// A detection failed.
        DetectionFailed = "pipeline.detection.failed",
        /// A redaction was created.
        RedactionCreated = "pipeline.redaction.created",
        /// A policy was created.
        PolicyCreated = "policy.created",
        /// A policy was updated.
        PolicyUpdated = "policy.updated",
        /// A policy was deleted.
        PolicyDeleted = "policy.deleted",
        /// A thread was opened.
        ThreadOpened = "thread.opened",
        /// A thread was closed.
        ThreadClosed = "thread.closed",
        /// A thread was reopened.
        ThreadReopened = "thread.reopened",
        /// A thread's title was changed.
        ThreadRenamed = "thread.renamed",
    }
}

impl WebhookEvent {
    /// Returns the event category as a string.
    pub fn category(&self) -> &'static str {
        match self {
            WebhookEvent::DocumentCreated
            | WebhookEvent::DocumentUpdated
            | WebhookEvent::DocumentDeleted => "document",
            WebhookEvent::ReviewVerified
            | WebhookEvent::ReviewAssigned
            | WebhookEvent::ReviewUnassigned => "review",
            WebhookEvent::MemberAdded
            | WebhookEvent::MemberDeleted
            | WebhookEvent::MemberUpdated => "member",
            WebhookEvent::ConnectionCreated
            | WebhookEvent::ConnectionUpdated
            | WebhookEvent::ConnectionDeleted
            | WebhookEvent::ConnectionSyncStarted
            | WebhookEvent::ConnectionSyncCompleted
            | WebhookEvent::ConnectionSyncFailed => "connection",
            WebhookEvent::ProviderCreated
            | WebhookEvent::ProviderUpdated
            | WebhookEvent::ProviderDeleted => "provider",
            WebhookEvent::PipelineCreated
            | WebhookEvent::PipelineUpdated
            | WebhookEvent::PipelineDeleted
            | WebhookEvent::DetectionStarted
            | WebhookEvent::DetectionCompleted
            | WebhookEvent::DetectionFailed
            | WebhookEvent::RedactionCreated => "pipeline",
            WebhookEvent::PolicyCreated
            | WebhookEvent::PolicyUpdated
            | WebhookEvent::PolicyDeleted => "policy",
            WebhookEvent::ThreadOpened
            | WebhookEvent::ThreadClosed
            | WebhookEvent::ThreadReopened
            | WebhookEvent::ThreadRenamed => "thread",
        }
    }

    /// Returns the event as a subject string for NATS routing.
    ///
    /// The event name is already a dotted, NATS-legal subject (e.g.
    /// `document.created`, `pipeline.redaction.created`), so this is the event's own
    /// string representation (from its `strum(serialize)`).
    pub fn as_subject(&self) -> &'static str {
        self.into()
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::WebhookEvent;

    #[test]
    fn category_is_the_tag_prefix_for_every_event() {
        // The category is exactly the first dotted segment of the event tag, so
        // it stays in sync with the wire form for every variant.
        for event in WebhookEvent::iter() {
            let tag = event.as_subject();
            let expected = tag.split('.').next().unwrap();
            assert_eq!(event.category(), expected, "{event:?}");
        }
    }

    #[test]
    fn as_subject_is_the_wire_tag() {
        assert_eq!(
            WebhookEvent::ProviderCreated.as_subject(),
            "provider.created"
        );
        assert_eq!(
            WebhookEvent::ConnectionSyncFailed.as_subject(),
            "connection.sync.failed"
        );
    }
}
