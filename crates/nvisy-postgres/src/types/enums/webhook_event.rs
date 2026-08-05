//! Webhook event type enumeration for webhook event subscriptions.

use diesel_derive_enum::DbEnum;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Defines the types of events that can trigger webhook delivery.
///
/// This enumeration corresponds to the `WEBHOOK_EVENT` PostgreSQL enum and is used
/// to configure which events a webhook should receive notifications for.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::WebhookEvent"]
pub enum WebhookEvent {
    // File events
    /// A new file was created
    #[db_rename = "file:created"]
    #[serde(rename = "file:created")]
    FileCreated,

    /// A file was updated
    #[db_rename = "file:updated"]
    #[serde(rename = "file:updated")]
    FileUpdated,

    /// A file was deleted
    #[db_rename = "file:deleted"]
    #[serde(rename = "file:deleted")]
    FileDeleted,

    // Member events
    /// A member was added to the workspace
    #[db_rename = "member:added"]
    #[serde(rename = "member:added")]
    MemberAdded,

    /// A member was deleted from the workspace
    #[db_rename = "member:deleted"]
    #[serde(rename = "member:deleted")]
    MemberDeleted,

    /// A member's details were updated
    #[db_rename = "member:updated"]
    #[serde(rename = "member:updated")]
    MemberUpdated,

    // Connection events
    /// A connection was created
    #[db_rename = "connection:created"]
    #[serde(rename = "connection:created")]
    ConnectionCreated,

    /// A connection was updated
    #[db_rename = "connection:updated"]
    #[serde(rename = "connection:updated")]
    ConnectionUpdated,

    /// A connection was deleted
    #[db_rename = "connection:deleted"]
    #[serde(rename = "connection:deleted")]
    ConnectionDeleted,

    /// A connection sync started
    #[db_rename = "connection:sync.started"]
    #[serde(rename = "connection:sync.started")]
    ConnectionSyncStarted,

    /// A connection sync finished successfully
    #[db_rename = "connection:sync.completed"]
    #[serde(rename = "connection:sync.completed")]
    ConnectionSyncCompleted,

    /// A connection sync failed
    #[db_rename = "connection:sync.failed"]
    #[serde(rename = "connection:sync.failed")]
    ConnectionSyncFailed,

    // Pipeline events
    /// A pipeline was created
    #[db_rename = "pipeline:created"]
    #[serde(rename = "pipeline:created")]
    PipelineCreated,

    /// A pipeline was updated
    #[db_rename = "pipeline:updated"]
    #[serde(rename = "pipeline:updated")]
    PipelineUpdated,

    /// A pipeline was deleted
    #[db_rename = "pipeline:deleted"]
    #[serde(rename = "pipeline:deleted")]
    PipelineDeleted,

    /// A pipeline run started
    #[db_rename = "pipeline:run.started"]
    #[serde(rename = "pipeline:run.started")]
    PipelineRunStarted,

    /// A pipeline run finished successfully
    #[db_rename = "pipeline:run.completed"]
    #[serde(rename = "pipeline:run.completed")]
    PipelineRunCompleted,

    /// A pipeline run failed
    #[db_rename = "pipeline:run.failed"]
    #[serde(rename = "pipeline:run.failed")]
    PipelineRunFailed,

    // Policy events
    /// A policy was created
    #[db_rename = "policy:created"]
    #[serde(rename = "policy:created")]
    PolicyCreated,

    /// A policy was updated
    #[db_rename = "policy:updated"]
    #[serde(rename = "policy:updated")]
    PolicyUpdated,

    /// A policy was deleted
    #[db_rename = "policy:deleted"]
    #[serde(rename = "policy:deleted")]
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
            | WebhookEvent::PipelineRunCompleted
            | WebhookEvent::PipelineRunFailed => "pipeline",
            WebhookEvent::PolicyCreated
            | WebhookEvent::PolicyUpdated
            | WebhookEvent::PolicyDeleted => "policy",
        }
    }

    /// Returns the event as a subject string for NATS routing.
    ///
    /// Format: `{category}.{action}` (e.g., "file.created", "member.deleted");
    /// run and sync events carry a nested action (e.g. "pipeline.run.completed",
    /// "connection.sync.started").
    pub fn as_subject(&self) -> &'static str {
        match self {
            WebhookEvent::FileCreated => "file.created",
            WebhookEvent::FileUpdated => "file.updated",
            WebhookEvent::FileDeleted => "file.deleted",
            WebhookEvent::MemberAdded => "member.added",
            WebhookEvent::MemberDeleted => "member.deleted",
            WebhookEvent::MemberUpdated => "member.updated",
            WebhookEvent::ConnectionCreated => "connection.created",
            WebhookEvent::ConnectionUpdated => "connection.updated",
            WebhookEvent::ConnectionDeleted => "connection.deleted",
            WebhookEvent::ConnectionSyncStarted => "connection.sync.started",
            WebhookEvent::ConnectionSyncCompleted => "connection.sync.completed",
            WebhookEvent::ConnectionSyncFailed => "connection.sync.failed",
            WebhookEvent::PipelineCreated => "pipeline.created",
            WebhookEvent::PipelineUpdated => "pipeline.updated",
            WebhookEvent::PipelineDeleted => "pipeline.deleted",
            WebhookEvent::PipelineRunStarted => "pipeline.run.started",
            WebhookEvent::PipelineRunCompleted => "pipeline.run.completed",
            WebhookEvent::PipelineRunFailed => "pipeline.run.failed",
            WebhookEvent::PolicyCreated => "policy.created",
            WebhookEvent::PolicyUpdated => "policy.updated",
            WebhookEvent::PolicyDeleted => "policy.deleted",
        }
    }
}
