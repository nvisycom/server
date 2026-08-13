//! Webhook event type enumeration for webhook event subscriptions.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoStaticStr};

/// Defines the types of events that can trigger webhook delivery.
///
/// This enumeration corresponds to the `WEBHOOK_EVENT` PostgreSQL enum and is used
/// to configure which events a webhook should receive notifications for.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(DbEnum, Display, EnumIter, EnumString, IntoStaticStr)]
#[ExistingTypePath = "crate::schema::sql_types::WebhookEvent"]
pub enum WebhookEvent {
    // File events
    /// A new file was created
    #[db_rename = "file.created"]
    #[serde(rename = "file.created")]
    #[strum(serialize = "file.created")]
    FileCreated,

    /// A file was updated
    #[db_rename = "file.updated"]
    #[serde(rename = "file.updated")]
    #[strum(serialize = "file.updated")]
    FileUpdated,

    /// A file was deleted
    #[db_rename = "file.deleted"]
    #[serde(rename = "file.deleted")]
    #[strum(serialize = "file.deleted")]
    FileDeleted,

    // Member events
    /// A member was added to the workspace
    #[db_rename = "member.added"]
    #[serde(rename = "member.added")]
    #[strum(serialize = "member.added")]
    MemberAdded,

    /// A member was deleted from the workspace
    #[db_rename = "member.deleted"]
    #[serde(rename = "member.deleted")]
    #[strum(serialize = "member.deleted")]
    MemberDeleted,

    /// A member's details were updated
    #[db_rename = "member.updated"]
    #[serde(rename = "member.updated")]
    #[strum(serialize = "member.updated")]
    MemberUpdated,

    // Connection events
    /// A connection was created
    #[db_rename = "connection.created"]
    #[serde(rename = "connection.created")]
    #[strum(serialize = "connection.created")]
    ConnectionCreated,

    /// A connection was updated
    #[db_rename = "connection.updated"]
    #[serde(rename = "connection.updated")]
    #[strum(serialize = "connection.updated")]
    ConnectionUpdated,

    /// A connection was deleted
    #[db_rename = "connection.deleted"]
    #[serde(rename = "connection.deleted")]
    #[strum(serialize = "connection.deleted")]
    ConnectionDeleted,

    /// A connection sync started
    #[db_rename = "connection.sync.started"]
    #[serde(rename = "connection.sync.started")]
    #[strum(serialize = "connection.sync.started")]
    ConnectionSyncStarted,

    /// A connection sync finished successfully
    #[db_rename = "connection.sync.completed"]
    #[serde(rename = "connection.sync.completed")]
    #[strum(serialize = "connection.sync.completed")]
    ConnectionSyncCompleted,

    /// A connection sync failed
    #[db_rename = "connection.sync.failed"]
    #[serde(rename = "connection.sync.failed")]
    #[strum(serialize = "connection.sync.failed")]
    ConnectionSyncFailed,

    // Pipeline events
    /// A pipeline was created
    #[db_rename = "pipeline.created"]
    #[serde(rename = "pipeline.created")]
    #[strum(serialize = "pipeline.created")]
    PipelineCreated,

    /// A pipeline was updated
    #[db_rename = "pipeline.updated"]
    #[serde(rename = "pipeline.updated")]
    #[strum(serialize = "pipeline.updated")]
    PipelineUpdated,

    /// A pipeline was deleted
    #[db_rename = "pipeline.deleted"]
    #[serde(rename = "pipeline.deleted")]
    #[strum(serialize = "pipeline.deleted")]
    PipelineDeleted,

    /// A pipeline run started
    #[db_rename = "pipeline.run.started"]
    #[serde(rename = "pipeline.run.started")]
    #[strum(serialize = "pipeline.run.started")]
    PipelineRunStarted,

    /// A pipeline run's detection finished (findings ready for review)
    #[db_rename = "pipeline.run.analyzed"]
    #[serde(rename = "pipeline.run.analyzed")]
    #[strum(serialize = "pipeline.run.analyzed")]
    PipelineRunAnalyzed,

    /// A pipeline run finished successfully
    #[db_rename = "pipeline.run.completed"]
    #[serde(rename = "pipeline.run.completed")]
    #[strum(serialize = "pipeline.run.completed")]
    PipelineRunCompleted,

    /// A pipeline run failed
    #[db_rename = "pipeline.run.failed"]
    #[serde(rename = "pipeline.run.failed")]
    #[strum(serialize = "pipeline.run.failed")]
    PipelineRunFailed,

    // Policy events
    /// A policy was created
    #[db_rename = "policy.created"]
    #[serde(rename = "policy.created")]
    #[strum(serialize = "policy.created")]
    PolicyCreated,

    /// A policy was updated
    #[db_rename = "policy.updated"]
    #[serde(rename = "policy.updated")]
    #[strum(serialize = "policy.updated")]
    PolicyUpdated,

    /// A policy was deleted
    #[db_rename = "policy.deleted"]
    #[serde(rename = "policy.deleted")]
    #[strum(serialize = "policy.deleted")]
    PolicyDeleted,
}

impl WebhookEvent {
    /// Returns whether this is a file-related event.
    #[inline]
    pub fn is_file_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::FileCreated | WebhookEvent::FileUpdated | WebhookEvent::FileDeleted
        )
    }

    /// Returns whether this is a member-related event.
    #[inline]
    pub fn is_member_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::MemberAdded | WebhookEvent::MemberDeleted | WebhookEvent::MemberUpdated
        )
    }

    /// Returns whether this is a connection-related event.
    #[inline]
    pub fn is_connection_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::ConnectionCreated
                | WebhookEvent::ConnectionUpdated
                | WebhookEvent::ConnectionDeleted
                | WebhookEvent::ConnectionSyncStarted
                | WebhookEvent::ConnectionSyncCompleted
                | WebhookEvent::ConnectionSyncFailed
        )
    }

    /// Returns whether this is a pipeline-related event (config or run).
    #[inline]
    pub fn is_pipeline_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::PipelineCreated
                | WebhookEvent::PipelineUpdated
                | WebhookEvent::PipelineDeleted
                | WebhookEvent::PipelineRunStarted
                | WebhookEvent::PipelineRunAnalyzed
                | WebhookEvent::PipelineRunCompleted
                | WebhookEvent::PipelineRunFailed
        )
    }

    /// Returns whether this is a policy-related event.
    #[inline]
    pub fn is_policy_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::PolicyCreated | WebhookEvent::PolicyUpdated | WebhookEvent::PolicyDeleted
        )
    }

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
            WebhookEvent::PipelineCreated
            | WebhookEvent::PipelineUpdated
            | WebhookEvent::PipelineDeleted
            | WebhookEvent::PipelineRunStarted
            | WebhookEvent::PipelineRunAnalyzed
            | WebhookEvent::PipelineRunCompleted
            | WebhookEvent::PipelineRunFailed => "pipeline",
            WebhookEvent::PolicyCreated
            | WebhookEvent::PolicyUpdated
            | WebhookEvent::PolicyDeleted => "policy",
        }
    }

    /// Returns the event as a subject string for NATS routing.
    ///
    /// The event name is already a dotted, NATS-legal subject (e.g.
    /// `file.created`, `pipeline.run.completed`), so this is the event's own
    /// string representation (from its `strum(serialize)`).
    pub fn as_subject(&self) -> &'static str {
        self.into()
    }
}
