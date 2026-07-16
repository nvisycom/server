//! Connection request types.

use nvisy_postgres::types::Slug;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Path parameters for connection operations.
///
/// The workspace is resolved separately from the `{workspaceSlug}` segment by
/// the [`WorkspaceContext`] extractor.
///
/// [`WorkspaceContext`]: crate::extract::WorkspaceContext
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionPathParams {
    /// URL slug of the connection, unique within its workspace.
    pub connection_slug: String,
}

/// Request payload for creating a new workspace connection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnection {
    /// Human-readable connection name.
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    /// URL slug, unique within the workspace and immutable after creation.
    pub slug: Slug,
    /// Provider type (e.g., "openai", "postgres", "s3").
    #[validate(length(min = 1, max = 64))]
    pub provider: String,
    /// Connection data to be encrypted (credentials + context).
    /// The structure depends on the provider type.
    pub data: serde_json::Value,
}

/// Request payload for updating an existing workspace connection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnection {
    /// Human-readable connection name.
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// Connection data to be encrypted (credentials + context).
    /// If provided, replaces the existing encrypted data.
    pub data: Option<serde_json::Value>,
}

/// Query parameters for listing connections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionsQuery {
    /// Filter by provider type.
    pub provider: Option<String>,
}
