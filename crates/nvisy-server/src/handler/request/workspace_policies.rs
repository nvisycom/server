//! Policy request types.

use elide_pipeline::entity::{LabelCatalog, LabelRef};
use elide_pipeline::governance::policy::{LabelScope, PolicyRule};
use elide_pipeline::governance::redaction::ModalityRedactions;
use elide_pipeline::template::PolicyTemplate;
use garde::Validate;
use nvisy_postgres::types::PolicyKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::input::{
    CreatePolicyInput, PolicyBodyInput, PolicyDraftInput, UpdatePolicyInput,
};

/// Query parameters for listing policies.
///
/// Every field is an optional filter; unset fields impose no constraint.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePoliciesQuery {
    /// Narrow the list to a single policy kind (`authored` or `oneshot`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PolicyKind>,
}

/// Path parameters for policy operations.
///
/// The workspace is resolved by the [`WorkspaceContext`] extractor from the
/// `{workspaceId}` path segment.
///
/// [`WorkspaceContext`]: crate::extract::WorkspaceContext
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePolicyPathParams {
    /// Id of the policy.
    pub policy_id: uuid::Uuid,
}

/// A client-authored policy body: the parts of a policy definition a caller may
/// set, without the fields the server owns.
///
/// The engine's `Policy` also carries an `id` and a `template` origin. Both are
/// server-owned — the `id` is minted at creation and the `template` records which
/// built-in a policy was seeded from (provenance). Neither is representable here,
/// so a client cannot mint ids or forge provenance; the server stamps them when it
/// builds the engine definition.
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
    /// Ordered rules. First match wins within this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,
    /// Per-policy catch-all, fired when no rule matched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<ModalityRedactions>,
}

impl From<PolicyDraft> for PolicyDraftInput {
    fn from(draft: PolicyDraft) -> Self {
        PolicyDraftInput {
            name: draft.name,
            description: draft.description,
            scopes: draft.scopes,
            rules: draft.rules,
            fallback: draft.fallback,
        }
    }
}

/// Where a new policy's body comes from: exactly one source, enforced by the
/// type so neither-nor-both is unrepresentable.
///
/// Tagged by `source`: `{ "source": "template", "template": { ... } }`,
/// `{ "source": "inline", "definition": { ... } }`, or
/// `{ "source": "labels", "labels": [ ... ] }`.
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
    /// A one-shot body: a bare list of labels, each erased. The created policy is
    /// content-addressed and deduplicated (an identical one is reused) and is
    /// immutable once minted. This is the ad-hoc redact flow.
    Labels {
        /// The built-in labels to detect and erase (e.g. `person_name`,
        /// `email_address`).
        labels: Vec<String>,
    },
}

impl From<PolicyBody> for PolicyBodyInput {
    fn from(body: PolicyBody) -> Self {
        match body {
            PolicyBody::Template { template } => PolicyBodyInput::Template { template },
            PolicyBody::Inline { definition } => PolicyBodyInput::Inline {
                definition: Box::new((*definition).into()),
            },
            PolicyBody::Labels { labels } => PolicyBodyInput::Labels { labels },
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
#[garde(allow_unvalidated)]
pub struct CreateWorkspacePolicy {
    /// Optional display name override. Defaults to the policy's own name. Ignored
    /// for a one-shot (labels) body, whose name is generated.
    #[garde(length(chars, min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Optional description override. Defaults to the policy's own description.
    #[garde(length(chars, max = 4096))]
    pub description: Option<String>,
    /// The source of the policy body.
    #[serde(flatten)]
    #[garde(custom(validate_body))]
    pub body: PolicyBody,
}

impl From<CreateWorkspacePolicy> for CreatePolicyInput {
    fn from(request: CreateWorkspacePolicy) -> Self {
        CreatePolicyInput {
            display_name: request.display_name,
            description: request.description,
            body: request.body.into(),
        }
    }
}

/// A one-shot (labels) body must name at least one label and no more than 64.
// reason: signature is fixed by garde's custom validator interface.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn validate_body(body: &PolicyBody, _: &()) -> garde::Result {
    if let PolicyBody::Labels { labels } = body {
        if labels.is_empty() {
            return Err(garde::Error::new("at least one label is required"));
        }
        if labels.len() > 64 {
            return Err(garde::Error::new("at most 64 labels are allowed"));
        }
        let catalog = LabelCatalog::with_builtins();
        let unknown: Vec<&str> = labels
            .iter()
            .filter(|id| !catalog.contains(&LabelRef::new(id.as_str())))
            .map(String::as_str)
            .collect();
        if !unknown.is_empty() {
            return Err(garde::Error::new(format!(
                "unknown labels: {}",
                unknown.join(", ")
            )));
        }
    }
    Ok(())
}

/// Request payload for updating an existing workspace policy.
///
/// Replacing the `definition` replaces the whole policy body. The policy's
/// template origin is server-owned and preserved across updates — it is not
/// settable here.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct UpdateWorkspacePolicy {
    /// Human-readable policy display name.
    #[garde(length(chars, min = 1, max = 255))]
    pub display_name: Option<String>,
    /// Policy description.
    #[garde(inner(inner(length(chars, max = 4096))))]
    pub description: Option<Option<String>>,
    /// New policy body (replaces the stored definition).
    pub definition: Option<PolicyDraft>,
}

impl From<UpdateWorkspacePolicy> for UpdatePolicyInput {
    fn from(request: UpdateWorkspacePolicy) -> Self {
        UpdatePolicyInput {
            display_name: request.display_name,
            description: request.description,
            definition: request.definition.map(Into::into),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_body_accepts_known_labels_and_rejects_unknown() {
        let known = PolicyBody::Labels {
            labels: vec!["person_name".to_owned(), "email_address".to_owned()],
        };
        assert!(validate_body(&known, &()).is_ok());

        let unknown = PolicyBody::Labels {
            labels: vec!["person_name".to_owned(), "not_a_real_label".to_owned()],
        };
        let err = validate_body(&unknown, &()).expect_err("unknown label is rejected");
        assert!(err.to_string().contains("not_a_real_label"));
    }
}
