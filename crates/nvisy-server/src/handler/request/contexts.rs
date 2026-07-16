//! Context request types.

use nvisy_schema::context::Context as SchemaContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Path parameters for context operations.
///
/// The workspace is resolved by the [`WorkspaceContext`] extractor from the
/// `{workspaceSlug}` path segment.
///
/// [`WorkspaceContext`]: crate::extract::WorkspaceContext
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextPathParams {
    /// Unique identifier of the context.
    pub context_id: Uuid,
}

/// Request payload for creating a new workspace context.
///
/// The `definition` is a structured context the redaction engine consumes;
/// its `name`, `description`, and `version` drive the stored columns unless
/// overridden here.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateContext {
    /// Optional display name override. Defaults to the context's own name.
    pub name: Option<String>,
    /// Optional description override. Defaults to the context's own description.
    pub description: Option<String>,
    /// The structured context body consumed by the engine.
    pub definition: SchemaContext,
}

/// Request payload for updating an existing workspace context.
///
/// Replacing the `definition` replaces the whole context body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateContext {
    /// Human-readable context name.
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// Context description.
    #[validate(length(max = 4096))]
    pub description: Option<Option<String>>,
    /// New context body (replaces the stored definition).
    pub definition: Option<SchemaContext>,
}
