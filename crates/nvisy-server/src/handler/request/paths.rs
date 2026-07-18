//! Path parameter types for HTTP handlers.

use nvisy_postgres::types::Username;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Path parameters for workspace member operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberPathParams {
    /// Public handle of the member's account.
    pub username: Username,
}

/// Path parameters for invite operations.
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

/// Path parameters for file operations within a workspace context.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFilePathParams {
    /// Unique identifier of the file.
    pub file_id: Uuid,
}

/// Path parameters for webhook operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookPathParams {
    /// URL slug of the webhook, unique within its workspace.
    pub webhook_slug: String,
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
/// Used when retrieving account information by handle. Access is granted
/// if the requester shares at least one workspace with the target account.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountPathParams {
    /// Public handle of the account.
    pub username: Username,
}

/// Path parameters for pipeline operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelinePathParams {
    /// URL slug of the pipeline, unique within its workspace.
    pub pipeline_slug: String,
}

/// Path parameters for pipeline run operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRunPathParams {
    /// URL slug of the pipeline the run belongs to.
    pub pipeline_slug: String,
    /// Per-pipeline sequential run number.
    pub run_number: i32,
}
