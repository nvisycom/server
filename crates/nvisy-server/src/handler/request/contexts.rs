//! Context request types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Path parameters for context operations.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextPathParams {
    /// Unique identifier of the context.
    pub context_id: Uuid,
}

/// Request payload for updating an existing workspace context.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateContext {
    /// Human-readable context name.
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// Context description.
    #[validate(length(max = 4096))]
    pub description: Option<Option<String>>,
}
