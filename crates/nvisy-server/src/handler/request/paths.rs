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

/// Path parameters for document operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPathParams {
    /// Unique identifier of the document.
    pub document_id: Uuid,
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

/// Path parameters for workspace invite operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InvitePathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
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

/// Path parameters for workspace integration operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationPathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// Unique identifier of the integration.
    pub integration_id: Uuid,
}

/// Path parameters for file operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilePathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
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

/// Path parameters for file comment operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileCommentPathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// Unique identifier of the file.
    pub file_id: Uuid,
    /// Unique identifier of the comment.
    pub comment_id: Uuid,
}

/// Path parameters for workspace webhook operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookPathParams {
    /// Unique identifier of the workspace.
    pub workspace_id: Uuid,
    /// Unique identifier of the webhook.
    pub webhook_id: Uuid,
}
