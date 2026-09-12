//! Policy request types.

use elide_pipeline::entity::{LabelCatalog, LabelRef};
use elide_pipeline::governance::policy::{LabelScope, Policy, PolicyRule, TemplateOrigin};
use elide_pipeline::governance::redaction::{ModalityRedactions, TextRedaction};
use elide_pipeline::template::PolicyTemplate;
use garde::Validate;
use nvisy_postgres::types::Handle;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Path parameters for policy operations.
///
/// The workspace is resolved by the [`WorkspaceContext`] extractor from the
/// `{workspaceSlug}` path segment.
///
/// [`WorkspaceContext`]: crate::extract::WorkspaceContext
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePolicyPathParams {
    /// URL slug of the policy, unique within its workspace.
    pub policy_slug: String,
}

/// A client-authored policy body: the parts of a policy definition a caller may
/// set, without the fields the server owns.
///
/// The engine's `Policy` also carries an `id` and a `template` origin. Both are
/// server-owned — the `id` is minted at creation and the `template` records which
/// built-in a policy was seeded from (provenance). Neither is representable here,
/// so a client cannot mint ids or forge provenance; the server stamps them in
/// [`into_definition`](PolicyDraft::into_definition).
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

impl PolicyDraft {
    /// Builds a full engine [`Policy`] from this draft, stamping the server-owned
    /// fields: a fresh `id`, and the given `template` origin (`None` for a
    /// hand-authored body, the built-in's origin when seeded from a template).
    pub fn into_definition(self, template: Option<TemplateOrigin>) -> Policy {
        Policy {
            id: Uuid::now_v7(),
            name: self.name.into(),
            description: self.description.map(Into::into),
            template,
            scopes: self.scopes,
            rules: self.rules,
            fallback: self.fallback,
        }
    }
}

/// The blanket redaction action a one-shot policy applies to every label it
/// detects. A one-shot trades per-label control for a single default; a full
/// policy is the tier for per-label actions and rules.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OneshotAction {
    /// Replace each detected value with a mask (`*`), keeping its shape.
    #[default]
    Mask,
    /// Delete each detected value entirely.
    Erase,
}

impl OneshotAction {
    /// The engine text redaction this action maps to.
    fn text_redaction(self) -> TextRedaction {
        match self {
            OneshotAction::Mask => TextRedaction::Mask {
                mask_char: '*',
                keep_prefix: 0,
                keep_suffix: 0,
            },
            OneshotAction::Erase => TextRedaction::Erase,
        }
    }

    /// A stable one-byte tag folded into the one-shot content hash, so two
    /// one-shots over the same labels but different actions hash distinctly.
    fn hash_tag(self) -> u8 {
        match self {
            OneshotAction::Mask => 0,
            OneshotAction::Erase => 1,
        }
    }
}

/// Where a new policy's body comes from: exactly one source, enforced by the
/// type so neither-nor-both is unrepresentable.
///
/// Tagged by `source`: `{ "source": "template", "template": { ... } }`,
/// `{ "source": "inline", "definition": { ... } }`, or
/// `{ "source": "labels", "labels": [ ... ], "action": "mask" }`.
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
    /// A one-shot body: a bare list of labels to redact with a single blanket
    /// action. The created policy is temporary — hidden from the list and not
    /// attachable to a pipeline until promoted. This is the ad-hoc redact flow.
    Labels {
        /// The built-in labels to detect and redact (e.g. `person_name`,
        /// `email_address`).
        labels: Vec<String>,
        /// The blanket action applied to every label. Defaults to masking.
        #[serde(default)]
        action: OneshotAction,
    },
}

impl PolicyBody {
    /// Whether this body creates a one-shot policy. Only the labels source does.
    pub fn is_oneshot(&self) -> bool {
        matches!(self, PolicyBody::Labels { .. })
    }

    /// The content-address hash of a one-shot body, or `None` for template/inline.
    ///
    /// A one-shot is identified by its semantic content: the set of labels and the
    /// blanket action. Labels are sorted and deduplicated first so the same content
    /// hashes equally regardless of request order, letting an identical one-shot be
    /// reused instead of duplicated. The minted definition `id` is deliberately not
    /// part of the hash.
    pub fn oneshot_content_hash(&self) -> Option<Vec<u8>> {
        let PolicyBody::Labels { labels, action } = self else {
            return None;
        };
        let mut ids: Vec<&str> = labels.iter().map(String::as_str).collect();
        ids.sort_unstable();
        ids.dedup();

        let mut hasher = Sha256::new();
        hasher.update([action.hash_tag()]);
        for id in ids {
            hasher.update((id.len() as u32).to_le_bytes());
            hasher.update(id.as_bytes());
        }
        Some(hasher.finalize().to_vec())
    }

    /// Resolves the body source into a concrete policy definition with a fresh
    /// `id`, so two policies seeded from the same template stay independent.
    ///
    /// An inline body is hand-authored, so it carries no template origin; a
    /// template body keeps the template's own origin (stamped by `build`). A
    /// labels body builds a degenerate definition — one scope over the picked
    /// labels plus a blanket fallback action — under the given generated name.
    pub fn into_definition(self, generated_name: &str) -> Policy {
        match self {
            PolicyBody::Inline { definition } => definition.into_definition(None),
            PolicyBody::Template { template } => Policy {
                // `build()` bakes a stable constant id; re-mint so each created
                // policy is distinct.
                id: Uuid::now_v7(),
                ..template.build().policy
            },
            PolicyBody::Labels { labels, action } => {
                let refs = labels.into_iter().map(LabelRef::new);
                Policy {
                    id: Uuid::now_v7(),
                    name: generated_name.into(),
                    description: None,
                    template: None,
                    scopes: vec![LabelScope::new(generated_name.to_owned(), refs)],
                    rules: Vec::new(),
                    fallback: Some(ModalityRedactions::text(action.text_redaction())),
                }
            }
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
    /// URL slug, unique within the workspace and immutable after creation.
    /// Required for a template or inline body; ignored (and generated) for a
    /// one-shot (labels) body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<Handle>,
    /// Optional description override. Defaults to the policy's own description.
    #[garde(length(chars, max = 4096))]
    pub description: Option<String>,
    /// The source of the policy body.
    #[serde(flatten)]
    #[garde(custom(validate_body))]
    pub body: PolicyBody,
}

/// A one-shot (labels) body must name at least one label and no more than 64.
fn validate_body(body: &PolicyBody, _: &()) -> garde::Result {
    if let PolicyBody::Labels { labels, .. } = body {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_body_builds_a_degenerate_definition() {
        let body = PolicyBody::Labels {
            labels: vec!["person_name".to_owned(), "email_address".to_owned()],
            action: OneshotAction::Mask,
        };
        assert!(body.is_oneshot());

        let definition = body.into_definition("Quick redaction abc123");
        assert_eq!(&*definition.name, "Quick redaction abc123");
        assert!(definition.rules.is_empty());
        assert_eq!(definition.scopes.len(), 1);
        let labels: Vec<&str> = definition.scopes[0]
            .labels
            .iter()
            .map(LabelRef::as_str)
            .collect();
        assert_eq!(labels, vec!["person_name", "email_address"]);
        assert!(
            matches!(
                definition.fallback.as_ref().and_then(|f| f.text.as_ref()),
                Some(TextRedaction::Mask { .. })
            ),
            "mask action maps to a text Mask fallback"
        );
    }

    #[test]
    fn erase_action_maps_to_erase() {
        let body = PolicyBody::Labels {
            labels: vec!["ip_address".to_owned()],
            action: OneshotAction::Erase,
        };
        let definition = body.into_definition("Quick redaction def456");
        assert!(matches!(
            definition.fallback.as_ref().and_then(|f| f.text.as_ref()),
            Some(TextRedaction::Erase)
        ));
    }

    #[test]
    fn template_and_inline_bodies_are_not_temporary() {
        let inline = PolicyBody::Inline {
            definition: Box::new(PolicyDraft {
                name: "Custom".to_owned(),
                description: None,
                scopes: Vec::new(),
                rules: Vec::new(),
                fallback: None,
            }),
        };
        assert!(!inline.is_oneshot());
    }

    #[test]
    fn oneshot_hash_is_order_insensitive_and_action_sensitive() {
        let a = PolicyBody::Labels {
            labels: vec!["person_name".to_owned(), "email_address".to_owned()],
            action: OneshotAction::Mask,
        };
        let b = PolicyBody::Labels {
            labels: vec!["email_address".to_owned(), "person_name".to_owned()],
            action: OneshotAction::Mask,
        };
        assert_eq!(a.oneshot_content_hash(), b.oneshot_content_hash());

        let erase = PolicyBody::Labels {
            labels: vec!["person_name".to_owned(), "email_address".to_owned()],
            action: OneshotAction::Erase,
        };
        assert_ne!(a.oneshot_content_hash(), erase.oneshot_content_hash());

        let inline = PolicyBody::Inline {
            definition: Box::new(PolicyDraft {
                name: "Custom".to_owned(),
                description: None,
                scopes: Vec::new(),
                rules: Vec::new(),
                fallback: None,
            }),
        };
        assert!(inline.oneshot_content_hash().is_none());
    }

    #[test]
    fn validate_body_accepts_known_labels_and_rejects_unknown() {
        let known = PolicyBody::Labels {
            labels: vec!["person_name".to_owned(), "email_address".to_owned()],
            action: OneshotAction::Mask,
        };
        assert!(validate_body(&known, &()).is_ok());

        let unknown = PolicyBody::Labels {
            labels: vec!["person_name".to_owned(), "not_a_real_label".to_owned()],
            action: OneshotAction::Mask,
        };
        let err = validate_body(&unknown, &()).expect_err("unknown label is rejected");
        assert!(err.to_string().contains("not_a_real_label"));
    }
}
