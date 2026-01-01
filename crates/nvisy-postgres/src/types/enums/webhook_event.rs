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
    // Document events
    /// A new document was created
    #[db_rename = "document:created"]
    #[serde(rename = "document:created")]
    DocumentCreated,

    /// A document was updated
    #[db_rename = "document:updated"]
    #[serde(rename = "document:updated")]
    DocumentUpdated,

    /// A document was deleted
    #[db_rename = "document:deleted"]
    #[serde(rename = "document:deleted")]
    DocumentDeleted,

    /// A document was processed
    #[db_rename = "document:processed"]
    #[serde(rename = "document:processed")]
    DocumentProcessed,

    /// A document was uploaded
    #[db_rename = "document:uploaded"]
    #[serde(rename = "document:uploaded")]
    DocumentUploaded,

    // Workspace events
    /// A workspace was updated
    #[db_rename = "workspace:updated"]
    #[serde(rename = "workspace:updated")]
    WorkspaceUpdated,

    /// A workspace was archived
    #[db_rename = "workspace:archived"]
    #[serde(rename = "workspace:archived")]
    WorkspaceArchived,

    // Member events
    /// A member was added to the workspace
    #[db_rename = "member:added"]
    #[serde(rename = "member:added")]
    MemberAdded,

    /// A member was removed from the workspace
    #[db_rename = "member:removed"]
    #[serde(rename = "member:removed")]
    MemberRemoved,

    /// A member's details were updated
    #[db_rename = "member:updated"]
    #[serde(rename = "member:updated")]
    MemberUpdated,

    // Integration events
    /// An integration was synchronized
    #[db_rename = "integration:synced"]
    #[serde(rename = "integration:synced")]
    IntegrationSynced,

    /// An integration failed
    #[db_rename = "integration:failed"]
    #[serde(rename = "integration:failed")]
    IntegrationFailed,

    // Run events
    /// A run was started
    #[db_rename = "run:started"]
    #[serde(rename = "run:started")]
    RunStarted,

    /// A run was completed successfully
    #[db_rename = "run:completed"]
    #[serde(rename = "run:completed")]
    RunCompleted,

    /// A run failed
    #[db_rename = "run:failed"]
    #[serde(rename = "run:failed")]
    RunFailed,
}

impl WebhookEvent {
    /// Returns whether this is a document-related event.
    #[inline]
    pub fn is_document_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::DocumentCreated
                | WebhookEvent::DocumentUpdated
                | WebhookEvent::DocumentDeleted
                | WebhookEvent::DocumentProcessed
                | WebhookEvent::DocumentUploaded
        )
    }

    /// Returns whether this is a workspace-related event.
    #[inline]
    pub fn is_workspace_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::WorkspaceUpdated | WebhookEvent::WorkspaceArchived
        )
    }

    /// Returns whether this is a member-related event.
    #[inline]
    pub fn is_member_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::MemberAdded | WebhookEvent::MemberRemoved | WebhookEvent::MemberUpdated
        )
    }

    /// Returns whether this is an integration-related event.
    #[inline]
    pub fn is_integration_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::IntegrationSynced | WebhookEvent::IntegrationFailed
        )
    }

    /// Returns whether this is a run-related event.
    #[inline]
    pub fn is_run_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::RunStarted | WebhookEvent::RunCompleted | WebhookEvent::RunFailed
        )
    }

    /// Returns the event category as a string.
    pub fn category(&self) -> &'static str {
        match self {
            WebhookEvent::DocumentCreated
            | WebhookEvent::DocumentUpdated
            | WebhookEvent::DocumentDeleted
            | WebhookEvent::DocumentProcessed
            | WebhookEvent::DocumentUploaded => "document",
            WebhookEvent::WorkspaceUpdated | WebhookEvent::WorkspaceArchived => "workspace",
            WebhookEvent::MemberAdded
            | WebhookEvent::MemberRemoved
            | WebhookEvent::MemberUpdated => "member",
            WebhookEvent::IntegrationSynced | WebhookEvent::IntegrationFailed => "integration",
            WebhookEvent::RunStarted | WebhookEvent::RunCompleted | WebhookEvent::RunFailed => {
                "run"
            }
        }
    }
}
