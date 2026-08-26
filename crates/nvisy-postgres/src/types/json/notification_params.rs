//! Notification payloads stored in `account_notifications.params`.
//!
//! A notification stores its `notify_type` (indexed column) plus a self-describing
//! [`Json`] body. [`NotificationPayload`] is that body — a `{type, data}`-tagged
//! enum, one variant per event, each carrying its own params struct. No rendered
//! text is stored; the client localizes copy from `type` and the params.

use serde::{Deserialize, Serialize};

use super::Json;
use crate::types::{ConnectionId, DetectionId, Handle, NotificationEvent, RedactionId};

/// Params of a `member.invited` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberInvitedParams {
    /// Slug of the workspace the account was invited to.
    pub workspace_slug: Handle,
    /// Username of the account that sent the invite, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invited_by: Option<Handle>,
}

/// Params of a `member.joined` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberJoinedParams {
    /// Slug of the workspace the member joined.
    pub workspace_slug: Handle,
    /// Username of the member that joined.
    pub member_username: Handle,
}

/// Params of a `connection.sync.completed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSyncCompletedParams {
    /// Id of the connection that synced.
    pub connection_id: ConnectionId,
    /// Display name of the connection that synced.
    pub connection_name: String,
    /// Number of records synced, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records_synced: Option<i64>,
}

/// Params of a `connection.sync.failed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSyncFailedParams {
    /// Id of the connection that failed to sync.
    pub connection_id: ConnectionId,
    /// Display name of the connection that failed to sync.
    pub connection_name: String,
    /// Failure reason, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Params of a `pipeline.detection.completed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetectionCompletedParams {
    /// Id of the detection.
    pub detection_id: DetectionId,
    /// Slug of the owning pipeline.
    pub pipeline_slug: Handle,
    /// Display name of the analyzed file, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_name: Option<String>,
}

/// Params of a `pipeline.redaction.created` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct RedactionCreatedParams {
    /// Id of the redaction.
    pub redaction_id: RedactionId,
    /// Id of the detection the redaction was produced from.
    pub detection_id: DetectionId,
    /// Slug of the owning pipeline.
    pub pipeline_slug: Handle,
    /// Display name of the redacted file, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_name: Option<String>,
}

/// Params of a `pipeline.detection.failed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct DetectionFailedParams {
    /// Id of the detection.
    pub detection_id: DetectionId,
    /// Slug of the owning pipeline.
    pub pipeline_slug: Handle,
    /// Display name of the analyzed file, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file_name: Option<String>,
    /// Failure reason, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The typed payload of a notification, tagged by `type` with its params under
/// `data` (the same `{type, data}` envelope the activity log and outbox event use).
///
/// Each variant is one notification type carrying its own params struct. The
/// `type` values match `NotificationEvent`, so the same key drives the member's
/// per-event preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "data")]
pub enum NotificationPayload {
    /// The account was invited to a workspace.
    #[serde(rename = "member.invited")]
    MemberInvited(MemberInvitedParams),

    /// A new member joined a workspace.
    #[serde(rename = "member.joined")]
    MemberJoined(MemberJoinedParams),

    /// A connection sync completed.
    #[serde(rename = "connection.sync.completed")]
    ConnectionSyncCompleted(ConnectionSyncCompletedParams),

    /// A connection sync failed.
    #[serde(rename = "connection.sync.failed")]
    ConnectionSyncFailed(ConnectionSyncFailedParams),

    /// A detection finished analysis and is ready to redact.
    #[serde(rename = "pipeline.detection.completed")]
    DetectionCompleted(DetectionCompletedParams),

    /// A redaction was created (redacted output produced).
    #[serde(rename = "pipeline.redaction.created")]
    RedactionCreated(RedactionCreatedParams),

    /// A detection failed.
    #[serde(rename = "pipeline.detection.failed")]
    DetectionFailed(DetectionFailedParams),
}

impl NotificationPayload {
    /// The [`NotificationEvent`] this payload is for (its `type` tag).
    pub fn event(&self) -> NotificationEvent {
        match self {
            NotificationPayload::MemberInvited(_) => NotificationEvent::MemberInvited,
            NotificationPayload::MemberJoined(_) => NotificationEvent::MemberJoined,
            NotificationPayload::ConnectionSyncCompleted(_) => {
                NotificationEvent::ConnectionSyncCompleted
            }
            NotificationPayload::ConnectionSyncFailed(_) => NotificationEvent::ConnectionSyncFailed,
            NotificationPayload::DetectionCompleted(_) => NotificationEvent::DetectionCompleted,
            NotificationPayload::RedactionCreated(_) => NotificationEvent::RedactionCreated,
            NotificationPayload::DetectionFailed(_) => NotificationEvent::DetectionFailed,
        }
    }

    /// Splits the payload into its event (for the indexed `notify_type` column)
    /// and the self-describing [`Json`] body stored in `params` — the tag
    /// stays in the body, so a read decodes it back symmetrically.
    pub fn into_stored(self) -> (NotificationEvent, Json<Self>) {
        let event = self.event();
        let params = Json::encode(&self);
        (event, params)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use uuid::Uuid;

    use super::*;
    use crate::types::DetectionId;

    #[test]
    fn serializes_as_a_type_data_envelope_and_round_trips() {
        let detection_id = DetectionId::from_uuid(Uuid::now_v7());
        let payload = NotificationPayload::DetectionCompleted(DetectionCompletedParams {
            detection_id,
            pipeline_slug: Handle::from_str("redact-invoices").unwrap(),
            input_file_name: Some("invoice.pdf".to_owned()),
        });

        let value = serde_json::to_value(&payload).unwrap();
        // The durable wire shape: a `type` tag and a nested `data` object, matching
        // the activity payload and outbox event. Stored rows depend on it.
        assert_eq!(value["type"], "pipeline.detection.completed");
        assert_eq!(value["data"]["detectionId"], detection_id.to_string());
        assert_eq!(value["data"]["pipelineSlug"], "redact-invoices");
        assert!(
            value.get("detectionId").is_none(),
            "params must nest under `data`"
        );

        let decoded: NotificationPayload = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.event(), NotificationEvent::DetectionCompleted);
    }
}
