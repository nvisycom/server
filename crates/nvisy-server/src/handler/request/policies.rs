//! Policy request types.

use nvisy_engine::policy::Policy as SchemaPolicy;
use nvisy_postgres::types::Handle;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Path parameters for policy operations.
///
/// The workspace is resolved by the [`WorkspaceContext`] extractor from the
/// `{workspaceSlug}` path segment.
///
/// [`WorkspaceContext`]: crate::extract::WorkspaceContext
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPathParams {
    /// URL slug of the policy, unique within its workspace.
    pub policy_slug: String,
}

/// Request payload for creating a new workspace policy.
///
/// The `definition` is a structured policy the redaction engine consumes;
/// its `name`, `description`, and `version` drive the stored columns unless
/// overridden here.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreatePolicy {
    /// Optional display name override. Defaults to the policy's own name.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// URL slug, unique within the workspace and immutable after creation.
    pub slug: Handle,
    /// Optional description override. Defaults to the policy's own description.
    #[validate(length(max = 4096))]
    pub description: Option<String>,
    /// The structured policy body consumed by the engine.
    pub definition: SchemaPolicy,
}

/// Request payload for updating an existing workspace policy.
///
/// Replacing the `definition` replaces the whole policy body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePolicy {
    /// Human-readable policy display name.
    #[validate(length(min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Policy description.
    #[validate(length(max = 4096))]
    pub description: Option<Option<String>>,
    /// New policy body (replaces the stored definition).
    pub definition: Option<SchemaPolicy>,
}
