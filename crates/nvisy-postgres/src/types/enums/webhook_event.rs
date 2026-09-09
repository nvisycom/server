//! Webhook event type enumeration for webhook event subscriptions.

use super::db_enum;

db_enum! {
    /// The types of events that can trigger webhook delivery.
    ///
    /// Corresponds to the `WEBHOOK_EVENT` PostgreSQL enum and configures which
    /// events a webhook receives.
    pub enum WebhookEvent = "crate::schema::sql_types::WebhookEvent" {
        /// A new file was created.
        FileCreated = "file.created",
        /// A file was updated.
        FileUpdated = "file.updated",
        /// A file was deleted.
        FileDeleted = "file.deleted",
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
    }
}

impl WebhookEvent {
    /// Returns the event category as a string.
    pub fn category(&self) -> &'static str {
        match self {
            WebhookEvent::FileCreated | WebhookEvent::FileUpdated | WebhookEvent::FileDeleted => {
                "file"
            }
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
        }
    }

    /// Returns the event as a subject string for NATS routing.
    ///
    /// The event name is already a dotted, NATS-legal subject (e.g.
    /// `file.created`, `pipeline.redaction.created`), so this is the event's own
    /// string representation (from its `strum(serialize)`).
    pub fn as_subject(&self) -> &'static str {
        self.into()
    }
}
