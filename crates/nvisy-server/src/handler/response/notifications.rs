//! Account notification response types.

use jiff::Timestamp;
use nvisy_postgres::model::AccountNotification;
use nvisy_postgres::types::NotificationEvent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;

/// Params of a `member:invited` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberInvitedParams {
    /// Slug of the workspace the account was invited to.
    pub workspace_slug: String,
    /// Username of the account that sent the invite, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invited_by: Option<String>,
}

/// Params of a `member:joined` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberJoinedParams {
    /// Slug of the workspace the member joined.
    pub workspace_slug: String,
    /// Username of the member that joined.
    pub member_username: String,
}

/// Params of a `connection:sync.completed` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSyncCompletedParams {
    /// Id of the connection that synced.
    pub connection_id: Uuid,
    /// Number of records synced, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records_synced: Option<i64>,
}

/// Params of a `connection:sync.failed` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSyncFailedParams {
    /// Id of the connection that failed to sync.
    pub connection_id: Uuid,
    /// Failure reason, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Params of a `pipeline:run.analyzed` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRunAnalyzedParams {
    /// Id of the run.
    pub run_id: Uuid,
    /// Slug of the owning pipeline.
    pub pipeline_slug: String,
    /// Display name of the analyzed file, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_name: Option<String>,
}

/// Params of a `pipeline:run.completed` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRunCompletedParams {
    /// Id of the run.
    pub run_id: Uuid,
    /// Slug of the owning pipeline.
    pub pipeline_slug: String,
    /// Display name of the analyzed file, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_name: Option<String>,
}

/// Params of a `pipeline:run.failed` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRunFailedParams {
    /// Id of the run.
    pub run_id: Uuid,
    /// Slug of the owning pipeline.
    pub pipeline_slug: String,
    /// Display name of the analyzed file, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_name: Option<String>,
    /// Failure reason, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Params of a `system:announcement` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemAnnouncementParams {
    /// Announcement message key or body.
    pub message: String,
}

/// Params of a `system:report` notification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemReportParams {
    /// Id of the generated report, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_id: Option<Uuid>,
}

/// The typed payload of a notification, tagged by `notifyType`.
///
/// Each variant is one notification type carrying its own params struct. No
/// rendered text is included — the client localizes the copy from `notifyType`
/// and the params. The `notifyType` values match the `NOTIFICATION_EVENT` enum,
/// so the same key drives the member's per-event preferences.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "notifyType", rename_all = "camelCase")]
pub enum NotificationPayload {
    /// The account was invited to a workspace.
    #[serde(rename = "member:invited")]
    MemberInvited(MemberInvitedParams),

    /// A new member joined a workspace.
    #[serde(rename = "member:joined")]
    MemberJoined(MemberJoinedParams),

    /// A connection sync completed.
    #[serde(rename = "connection:sync.completed")]
    ConnectionSyncCompleted(ConnectionSyncCompletedParams),

    /// A connection sync failed.
    #[serde(rename = "connection:sync.failed")]
    ConnectionSyncFailed(ConnectionSyncFailedParams),

    /// A pipeline run finished detection and is awaiting review.
    #[serde(rename = "pipeline:run.analyzed")]
    PipelineRunAnalyzed(PipelineRunAnalyzedParams),

    /// A pipeline run completed (redaction produced).
    #[serde(rename = "pipeline:run.completed")]
    PipelineRunCompleted(PipelineRunCompletedParams),

    /// A pipeline run failed.
    #[serde(rename = "pipeline:run.failed")]
    PipelineRunFailed(PipelineRunFailedParams),

    /// A system-wide announcement.
    #[serde(rename = "system:announcement")]
    SystemAnnouncement(SystemAnnouncementParams),

    /// A system report was generated.
    #[serde(rename = "system:report")]
    SystemReport(SystemReportParams),
}

impl NotificationPayload {
    /// The [`NotificationEvent`] this payload is for (its `notifyType`).
    pub fn event(&self) -> NotificationEvent {
        match self {
            NotificationPayload::MemberInvited(_) => NotificationEvent::MemberInvited,
            NotificationPayload::MemberJoined(_) => NotificationEvent::MemberJoined,
            NotificationPayload::ConnectionSyncCompleted(_) => {
                NotificationEvent::ConnectionSyncCompleted
            }
            NotificationPayload::ConnectionSyncFailed(_) => NotificationEvent::ConnectionSyncFailed,
            NotificationPayload::PipelineRunAnalyzed(_) => NotificationEvent::PipelineRunAnalyzed,
            NotificationPayload::PipelineRunCompleted(_) => NotificationEvent::PipelineRunCompleted,
            NotificationPayload::PipelineRunFailed(_) => NotificationEvent::PipelineRunFailed,
            NotificationPayload::SystemAnnouncement(_) => NotificationEvent::SystemAnnouncement,
            NotificationPayload::SystemReport(_) => NotificationEvent::SystemReport,
        }
    }

    /// Splits the payload into its event and its bare params (the tagged-enum
    /// object without the `notifyType` tag), as stored on the row.
    ///
    /// Serialization of an internally-tagged enum always yields a JSON object, so
    /// removing the tag leaves the params object.
    pub fn into_stored(self) -> (NotificationEvent, serde_json::Value) {
        let event = self.event();
        let mut value = serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}));
        if let serde_json::Value::Object(map) = &mut value {
            map.remove("notifyType");
        }
        (event, value)
    }
}

/// Response type for an account notification.
///
/// The typed [`NotificationPayload`] is nested under `payload`, so a notification
/// is `{ id, payload: { notifyType, <params...> }, isRead, ... }`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// Unique notification identifier.
    pub id: Uuid,
    /// The notification type and its typed params.
    pub payload: NotificationPayload,
    /// Whether the notification has been read.
    pub is_read: bool,
    /// When the notification was read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_at: Option<Timestamp>,
    /// When the notification was created.
    pub created_at: Timestamp,
    /// When the notification expires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
}

/// Paginated list of notifications.
pub type NotificationsPage = Page<Notification>;

/// Response type for unread notifications status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnreadStatus {
    /// Number of unread notifications.
    pub unread_count: i64,
}

/// Response type for a mark-all-read action.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkedReadStatus {
    /// Number of notifications the request marked as read.
    pub marked_read: i64,
}

impl Notification {
    /// Builds the response from a stored notification, reconstructing the typed
    /// payload from `notify_type` and the stored params.
    ///
    /// A row whose stored params do not match its `notify_type` (e.g. written by
    /// an older shape) is skipped by returning `None`, rather than failing the
    /// whole listing.
    pub fn from_model(notification: AccountNotification) -> Option<Self> {
        // Reconstruct the tagged payload: fold `notifyType` back in alongside the
        // stored params, then deserialize the whole into NotificationPayload.
        let mut tagged = notification.params.clone();
        if let serde_json::Value::Object(map) = &mut tagged {
            map.insert(
                "notifyType".to_owned(),
                serde_json::Value::String(notification.notify_type.to_string()),
            );
        }
        let payload: NotificationPayload = serde_json::from_value(tagged).ok()?;

        Some(Self {
            id: notification.id,
            payload,
            is_read: notification.is_read,
            read_at: notification.read_at.map(Into::into),
            created_at: notification.created_at.into(),
            expires_at: notification.expires_at.map(Into::into),
        })
    }
}
