//! Activity payloads stored in `workspace_activities.params`.
//!
//! An activity stores its `activity_type` (indexed column) plus a self-describing
//! [`Json`](super::Json) body. [`ActivityPayload`] is that body — an
//! `activityType`-tagged enum, one variant per event, each carrying its own
//! params. No rendered text is stored; the client localizes copy from
//! `activityType` and the params.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Handle;

/// Params of a workspace-scoped activity (`workspace.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceActivityParams {
    /// Slug of the workspace acted on.
    pub workspace_slug: Handle,
}

/// Params of a member activity (`member.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MemberActivityParams {
    /// Username of the member acted on.
    pub member_username: Handle,
}

/// Params of an invite activity (`invite.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct InviteActivityParams {
    /// Id of the invite.
    pub invite_id: Uuid,
    /// Email the invite was addressed to.
    pub email: String,
}

/// Params of a connection activity (`connection.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConnectionActivityParams {
    /// Id of the connection.
    pub connection_id: Uuid,
}

/// Params of a webhook activity (`webhook.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WebhookActivityParams {
    /// Id of the webhook.
    pub webhook_id: Uuid,
}

/// Params of a file activity (`file.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileActivityParams {
    /// Id of the file.
    pub file_id: Uuid,
    /// Display name of the file.
    pub file_name: String,
}

/// Params of a pipeline activity (`pipeline.*`, non-run).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PipelineActivityParams {
    /// Slug of the pipeline.
    pub pipeline_slug: Handle,
}

/// Params of a pipeline-run activity (`pipeline.run.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PipelineRunActivityParams {
    /// Slug of the owning pipeline.
    pub pipeline_slug: Handle,
    /// Id of the run.
    pub run_id: Uuid,
}

/// Params of a policy activity (`policy.*`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PolicyActivityParams {
    /// Id of the policy.
    pub policy_id: Uuid,
}

/// The typed payload of an audit-log activity, tagged by `activityType`.
///
/// Each variant is one activity type carrying its own params. No rendered text is
/// included — the client localizes the copy from `activityType` and the params.
/// The `activityType` values match the `ACTIVITY_TYPE` enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "activityType", rename_all = "camelCase")]
pub enum ActivityPayload {
    /// A workspace was created.
    #[serde(rename = "workspace.created")]
    WorkspaceCreated(WorkspaceActivityParams),
    /// A workspace was updated.
    #[serde(rename = "workspace.updated")]
    WorkspaceUpdated(WorkspaceActivityParams),
    /// A workspace was deleted.
    #[serde(rename = "workspace.deleted")]
    WorkspaceDeleted(WorkspaceActivityParams),

    /// A member joined the workspace.
    #[serde(rename = "member.added")]
    MemberAdded(MemberActivityParams),
    /// A member was updated.
    #[serde(rename = "member.updated")]
    MemberUpdated(MemberActivityParams),
    /// A member was removed.
    #[serde(rename = "member.deleted")]
    MemberDeleted(MemberActivityParams),

    /// An invite was created.
    #[serde(rename = "invite.created")]
    InviteCreated(InviteActivityParams),
    /// An invite was accepted.
    #[serde(rename = "invite.accepted")]
    InviteAccepted(InviteActivityParams),
    /// An invite was declined.
    #[serde(rename = "invite.declined")]
    InviteDeclined(InviteActivityParams),
    /// An invite was canceled.
    #[serde(rename = "invite.canceled")]
    InviteCanceled(InviteActivityParams),

    /// A connection was created.
    #[serde(rename = "connection.created")]
    ConnectionCreated(ConnectionActivityParams),
    /// A connection was updated.
    #[serde(rename = "connection.updated")]
    ConnectionUpdated(ConnectionActivityParams),
    /// A connection was deleted.
    #[serde(rename = "connection.deleted")]
    ConnectionDeleted(ConnectionActivityParams),
    /// A connection sync completed.
    #[serde(rename = "connection.sync.completed")]
    ConnectionSyncCompleted(ConnectionActivityParams),
    /// A connection sync failed.
    #[serde(rename = "connection.sync.failed")]
    ConnectionSyncFailed(ConnectionActivityParams),

    /// A webhook was created.
    #[serde(rename = "webhook.created")]
    WebhookCreated(WebhookActivityParams),
    /// A webhook was updated.
    #[serde(rename = "webhook.updated")]
    WebhookUpdated(WebhookActivityParams),
    /// A webhook was deleted.
    #[serde(rename = "webhook.deleted")]
    WebhookDeleted(WebhookActivityParams),
    /// A webhook was triggered.
    #[serde(rename = "webhook.triggered")]
    WebhookTriggered(WebhookActivityParams),

    /// A file was created.
    #[serde(rename = "file.created")]
    FileCreated(FileActivityParams),
    /// A file was updated.
    #[serde(rename = "file.updated")]
    FileUpdated(FileActivityParams),
    /// A file was deleted.
    #[serde(rename = "file.deleted")]
    FileDeleted(FileActivityParams),
    /// A file was verified.
    #[serde(rename = "file.verified")]
    FileVerified(FileActivityParams),

    /// A pipeline was created.
    #[serde(rename = "pipeline.created")]
    PipelineCreated(PipelineActivityParams),
    /// A pipeline was updated.
    #[serde(rename = "pipeline.updated")]
    PipelineUpdated(PipelineActivityParams),
    /// A pipeline was deleted.
    #[serde(rename = "pipeline.deleted")]
    PipelineDeleted(PipelineActivityParams),
    /// A pipeline run was started.
    #[serde(rename = "pipeline.run.started")]
    PipelineRunStarted(PipelineRunActivityParams),
    /// A pipeline run finished detection.
    #[serde(rename = "pipeline.run.analyzed")]
    PipelineRunAnalyzed(PipelineRunActivityParams),
    /// A pipeline run completed.
    #[serde(rename = "pipeline.run.completed")]
    PipelineRunCompleted(PipelineRunActivityParams),
    /// A pipeline run failed.
    #[serde(rename = "pipeline.run.failed")]
    PipelineRunFailed(PipelineRunActivityParams),

    /// A policy was created.
    #[serde(rename = "policy.created")]
    PolicyCreated(PolicyActivityParams),
    /// A policy was updated.
    #[serde(rename = "policy.updated")]
    PolicyUpdated(PolicyActivityParams),
    /// A policy was deleted.
    #[serde(rename = "policy.deleted")]
    PolicyDeleted(PolicyActivityParams),
}
