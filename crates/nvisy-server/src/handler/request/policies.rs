//! Policy request types.

use nvisy_schema::policy::Policy as SchemaPolicy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
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
    /// Unique identifier of the policy.
    pub policy_id: Uuid,
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
    pub name: Option<String>,
    /// Optional description override. Defaults to the policy's own description.
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
    /// Human-readable policy name.
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// Policy description.
    #[validate(length(max = 4096))]
    pub description: Option<Option<String>>,
    /// New policy body (replaces the stored definition).
    pub definition: Option<SchemaPolicy>,
}
