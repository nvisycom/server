//! Notification payloads stored in `account_notifications.params`.
//!
//! A notification stores its `notify_type` (indexed column) plus a self-describing
//! [`Json`] body. [`NotificationPayload`] is that body — a `notifyType`-tagged
//! enum, one variant per event, each carrying its own params struct. No rendered
//! text is stored; the client localizes copy from `notifyType` and the params.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Json;
use crate::types::NotificationEvent;

/// Params of a `member.invited` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberInvitedParams {
    /// Slug of the workspace the account was invited to.
    pub workspace_slug: String,
    /// Username of the account that sent the invite, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invited_by: Option<String>,
}

/// Params of a `member.joined` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberJoinedParams {
    /// Slug of the workspace the member joined.
    pub workspace_slug: String,
    /// Username of the member that joined.
    pub member_username: String,
}

/// Params of a `connection.sync.completed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSyncCompletedParams {
    /// Id of the connection that synced.
    pub connection_id: Uuid,
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
    pub connection_id: Uuid,
    /// Failure reason, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Params of a `pipeline.run.analyzed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// Params of a `pipeline.run.completed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// Params of a `pipeline.run.failed` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

/// Params of a `system.announcement` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemAnnouncementParams {
    /// Announcement message key or body.
    pub message: String,
}

/// Params of a `system.report` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SystemReportParams {
    /// Id of the generated report, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_id: Option<Uuid>,
}

/// The typed payload of a notification, tagged by `notifyType`.
///
/// Each variant is one notification type carrying its own params struct. The
/// `notifyType` values match [`NotificationEvent`], so the same key drives the
/// member's per-event preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "notifyType", rename_all = "camelCase")]
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

    /// A pipeline run finished detection and is awaiting review.
    #[serde(rename = "pipeline.run.analyzed")]
    PipelineRunAnalyzed(PipelineRunAnalyzedParams),

    /// A pipeline run completed (redaction produced).
    #[serde(rename = "pipeline.run.completed")]
    PipelineRunCompleted(PipelineRunCompletedParams),

    /// A pipeline run failed.
    #[serde(rename = "pipeline.run.failed")]
    PipelineRunFailed(PipelineRunFailedParams),

    /// A system-wide announcement.
    #[serde(rename = "system.announcement")]
    SystemAnnouncement(SystemAnnouncementParams),

    /// A system report was generated.
    #[serde(rename = "system.report")]
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

    /// Splits the payload into its event (for the indexed `notify_type` column)
    /// and the self-describing [`Json`] body stored in `params` — the tag
    /// stays in the body, so a read decodes it back symmetrically.
    pub fn into_stored(self) -> (NotificationEvent, Json<Self>) {
        let event = self.event();
        let params = Json::encode(&self);
        (event, params)
    }
}
