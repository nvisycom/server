//! Policy request types.

use nvisy_engine::policy::PolicyDefinition;
use nvisy_engine::template::PolicyTemplate;
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

/// Where a new policy's body comes from: exactly one source, enforced by the
/// type so neither-nor-both is unrepresentable.
///
/// Tagged by `source`: `{ "source": "template", "template": "hipaa_safe_harbor" }`
/// or `{ "source": "inline", "definition": { ... } }`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "camelCase")]
pub enum PolicyBody {
    /// Seed the body from a built-in policy template.
    ///
    /// The template's body is copied into a normal, independently-editable
    /// policy at creation time.
    Template {
        /// The built-in policy template to seed from.
        template: PolicyTemplate,
    },
    /// An inline structured policy body consumed by the engine.
    Inline {
        /// The structured policy body.
        ///
        /// Boxed to keep the enum small: an inline body is much larger than a
        /// template id, and most requests use a template.
        definition: Box<PolicyDefinition>,
    },
}

impl PolicyBody {
    /// Resolves the body source into a concrete policy definition: the inline
    /// body as-is, or the template's body materialized from the runtime.
    pub fn into_definition(self) -> PolicyDefinition {
        match self {
            PolicyBody::Inline { definition } => *definition,
            PolicyBody::Template { template } => template.build().policy,
        }
    }
}

/// Request payload for creating a new workspace policy.
///
/// The body comes from a template or an inline definition (see [`PolicyBody`]).
/// The body's `name` and `description` drive the stored columns unless
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
    /// The source of the policy body.
    #[serde(flatten)]
    pub body: PolicyBody,
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
    pub definition: Option<PolicyDefinition>,
}
