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

    // Integration events
    /// An integration was created
    #[db_rename = "integration:created"]
    #[serde(rename = "integration:created")]
    IntegrationCreated,

    /// An integration was updated
    #[db_rename = "integration:updated"]
    #[serde(rename = "integration:updated")]
    IntegrationUpdated,

    /// An integration was deleted
    #[db_rename = "integration:deleted"]
    #[serde(rename = "integration:deleted")]
    IntegrationDeleted,

    /// An integration was synchronized
    #[db_rename = "integration:synced"]
    #[serde(rename = "integration:synced")]
    IntegrationSynced,
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
        )
    }

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

    /// Returns whether this is an integration-related event.
    #[inline]
    pub fn is_integration_event(self) -> bool {
        matches!(
            self,
            WebhookEvent::IntegrationCreated
                | WebhookEvent::IntegrationUpdated
                | WebhookEvent::IntegrationDeleted
                | WebhookEvent::IntegrationSynced
        )
    }

    /// Returns the event category as a string.
    pub fn category(&self) -> &'static str {
        match self {
            WebhookEvent::DocumentCreated
            | WebhookEvent::DocumentUpdated
            | WebhookEvent::DocumentDeleted => "document",
            WebhookEvent::FileCreated | WebhookEvent::FileUpdated | WebhookEvent::FileDeleted => {
                "file"
            }
            WebhookEvent::MemberAdded
            | WebhookEvent::MemberDeleted
            | WebhookEvent::MemberUpdated => "member",
            WebhookEvent::IntegrationCreated
            | WebhookEvent::IntegrationUpdated
            | WebhookEvent::IntegrationDeleted
            | WebhookEvent::IntegrationSynced => "integration",
        }
    }
}
