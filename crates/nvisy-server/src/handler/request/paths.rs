//! Path parameter types for HTTP handlers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Path parameters for workspace-level operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
}

/// Path parameters for workspace member operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberPathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// Unique identifier of the member account.
    pub account_id: Uuid,
}

/// Path parameters for invite operations (invite ID only).
///
/// Since invite IDs are globally unique UUIDs, workspace context can be
/// derived from the invite record itself for authorization purposes.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InvitePathParams {
    /// Unique identifier of the invite.
    pub invite_id: Uuid,
}

/// Path parameters for joining via invite code.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InviteCodePathParams {
    /// The invite code to use for joining the workspace.
    pub invite_code: String,
}

/// Path parameters for integration operations (integration ID only).
///
/// Since integration IDs are globally unique UUIDs, workspace context can be
/// derived from the integration record itself for authorization purposes.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationPathParams {
    /// Unique identifier of the integration.
    pub integration_id: Uuid,
}

/// Path parameters for file operations within a workspace context.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFilePathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// Unique identifier of the file.
    pub file_id: Uuid,
}

/// Path parameters for file operations (file ID only).
///
/// Since file IDs are globally unique UUIDs, workspace context can be
/// derived from the file record itself for authorization purposes.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilePathParams {
    /// Unique identifier of the file.
    pub file_id: Uuid,
}

/// Path parameters for version operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionPathParams {
    /// Unique identifier of the version.
    pub version_id: Uuid,
}

/// Path parameters for webhook operations (webhook ID only).
///
/// Since webhook IDs are globally unique UUIDs, workspace context can be
/// derived from the webhook record itself for authorization purposes.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookPathParams {
    /// Unique identifier of the webhook.
    pub webhook_id: Uuid,
}

/// Path parameters for annotation operations (annotation ID only).
///
/// Since annotation IDs are globally unique UUIDs, file/workspace context can be
/// derived from the annotation record itself for authorization purposes.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationPathParams {
    /// Unique identifier of the annotation.
    pub annotation_id: Uuid,
}

/// Path parameters for integration run operations (run ID only).
///
/// Since run IDs are globally unique UUIDs, workspace context can be
/// derived from the run record itself for authorization purposes.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationRunPathParams {
    /// Unique identifier of the integration run.
    pub run_id: Uuid,
}

/// Path parameters for API token operations.
///
/// Since token IDs are globally unique UUIDs, account context is verified
/// by comparing with the authenticated user's account ID.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenPathParams {
    /// Unique identifier of the API token.
    pub token_id: Uuid,
}

/// Path parameters for account operations.
///
/// Used when retrieving account information by ID. Access is granted
/// if the requester shares at least one workspace with the target account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountPathParams {
    /// Unique identifier of the account.
    pub account_id: Uuid,
}

/// Path parameters for pipeline operations.
///
/// Since pipeline IDs are globally unique UUIDs, workspace context can be
/// derived from the pipeline record itself for authorization purposes.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelinePathParams {
    /// Unique identifier of the pipeline.
    pub pipeline_id: Uuid,
}

/// Path parameters for pipeline run operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRunPathParams {
    /// Unique identifier of the pipeline run.
    pub run_id: Uuid,
}
