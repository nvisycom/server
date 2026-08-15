//! Policy request types.

use elide_pipeline::entity::Label;
use elide_pipeline::policy::redaction::ModalityRedactions;
use elide_pipeline::policy::{LabelScope, PolicyDefinition, PolicyRule, TemplateOrigin};
use elide_pipeline::template::PolicyTemplate;
use nvisy_postgres::types::Handle;
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
    /// URL slug of the policy, unique within its workspace.
    pub policy_slug: String,
}

/// A client-authored policy body: the parts of a policy definition a caller may
/// set, without the fields the server owns.
///
/// The engine's `PolicyDefinition` also carries an `id` and a `template` origin.
/// Both are server-owned — the `id` is minted at creation and the `template`
/// records which built-in a policy was seeded from (provenance). Neither is
/// representable here, so a client cannot mint ids or forge provenance; the
/// server stamps them in [`into_definition`](PolicyDraft::into_definition).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDraft {
    /// Human-readable name. Display-only.
    pub name: String,
    /// Optional description for reviewers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What this policy detects: named, attributed label sets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<LabelScope>,
    /// Caller-authored custom label schemas this policy introduces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<Label>,
    /// Ordered rules. First match wins within this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,
    /// Per-policy catch-all, fired when no rule matched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<ModalityRedactions>,
}

impl PolicyDraft {
    /// Builds a full engine [`PolicyDefinition`] from this draft, stamping the
    /// server-owned fields: a fresh `id`, and the given `template` origin
    /// (`None` for a hand-authored body, the built-in's origin when seeded from
    /// a template).
    pub fn into_definition(self, template: Option<TemplateOrigin>) -> PolicyDefinition {
        PolicyDefinition {
            id: Uuid::now_v7(),
            name: self.name.into(),
            description: self.description.map(Into::into),
            template,
            scopes: self.scopes,
            custom: self.custom,
            rules: self.rules,
            fallback: self.fallback,
        }
    }
}

/// Where a new policy's body comes from: exactly one source, enforced by the
/// type so neither-nor-both is unrepresentable.
///
/// Tagged by `source`: `{ "source": "template", "template": { ... } }`
/// or `{ "source": "inline", "definition": { ... } }`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "camelCase")]
pub enum PolicyBody {
    /// Seed the body from a built-in policy template.
    ///
    /// The template's body is copied into a normal, independently-editable
    /// policy at creation time, tagged with the template's origin.
    Template {
        /// The built-in policy template to seed from.
        template: PolicyTemplate,
    },
    /// An inline structured policy body consumed by the engine.
    Inline {
        /// The client-authored policy body.
        ///
        /// Boxed to keep the enum small: an inline body is much larger than a
        /// template id, and most requests use a template.
        definition: Box<PolicyDraft>,
    },
}

impl PolicyBody {
    /// Resolves the body source into a concrete policy definition with a fresh
    /// `id`, so two policies seeded from the same template stay independent.
    ///
    /// An inline body is hand-authored, so it carries no template origin; a
    /// template body keeps the template's own origin (stamped by `build`).
    pub fn into_definition(self) -> PolicyDefinition {
        match self {
            PolicyBody::Inline { definition } => definition.into_definition(None),
            PolicyBody::Template { template } => PolicyDefinition {
                // `build()` bakes a stable constant id; re-mint so each created
                // policy is distinct.
                id: Uuid::now_v7(),
                ..template.build().policy
            },
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
/// Replacing the `definition` replaces the whole policy body. The policy's
/// template origin is server-owned and preserved across updates — it is not
/// settable here.
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
    pub definition: Option<PolicyDraft>,
}
