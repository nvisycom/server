//! Path parameter types for HTTP handlers.

use nvisy_postgres::types::{DetectionId, Handle, RedactionId, WebhookId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Path parameters for workspace member operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMemberPathParams {
    /// Public handle of the member's account.
    pub username: Handle,
}

/// Path parameters for invite operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInvitePathParams {
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

/// Path parameters for document operations within a workspace context.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDocumentPathParams {
    /// Unique identifier of the document.
    pub document_id: Uuid,
}

/// Path parameters for webhook operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWebhookPathParams {
    /// Opaque identifier of the webhook.
    pub webhook_id: WebhookId,
}

/// Path parameters for API token operations.
///
/// Since token IDs are globally unique UUIDs, account context is verified
/// by comparing with the authenticated user's account ID.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountApiTokenPathParams {
    /// Unique identifier of the API token.
    pub token_id: Uuid,
}

/// Path parameters for account operations.
///
/// Used when retrieving account information by handle. Access is granted
/// if the requester shares at least one workspace with the target account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountPathParams {
    /// Public handle of the account.
    pub username: Handle,
}

/// Path parameters for pipeline operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePipelinePathParams {
    /// URL slug of the pipeline, unique within its workspace.
    pub pipeline_slug: String,
}

/// Path parameters for detection operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetectionPathParams {
    /// Opaque identifier of the detection.
    pub detection_id: DetectionId,
}

/// Path parameters for a redaction.
///
/// The redaction id is globally unique, so a redaction is addressed by id alone
/// and resolved within the workspace by the query.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRedactionPathParams {
    /// Opaque identifier of the redaction.
    pub redaction_id: RedactionId,
}

/// Path parameters for notification operations.
///
/// The notification id is globally unique; account ownership is enforced in the
/// query, so a notification of another account resolves to a not-found result.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountNotificationPathParams {
    /// Unique identifier of the notification.
    pub notification_id: Uuid,
}
