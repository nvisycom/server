//! Policy service inputs.

use elide_pipeline::entity::LabelRef;
use elide_pipeline::governance::policy::{LabelScope, Policy, PolicyRule, TemplateOrigin};
use elide_pipeline::governance::redaction::{ModalityRedactions, TextRedaction};
use elide_pipeline::template::PolicyTemplate;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A hand-authored policy body: the parts of a definition a caller sets, without
/// the server-owned `id` and `template` origin.
#[derive(Clone)]
pub struct PolicyDraftInput {
    /// Human-readable name.
    pub name: String,
    /// Optional description for reviewers.
    pub description: Option<String>,
    /// What this policy detects: named, attributed label sets.
    pub scopes: Vec<LabelScope>,
    /// Ordered rules. First match wins within this policy.
    pub rules: Vec<PolicyRule>,
    /// Per-policy catch-all, fired when no rule matched.
    pub fallback: Option<ModalityRedactions>,
}

impl PolicyDraftInput {
    /// Builds a full engine [`Policy`], stamping a fresh `id` and the given
    /// `template` origin (`None` for a hand-authored body).
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

/// Where a new policy's body comes from: exactly one source.
#[derive(Clone)]
pub enum PolicyBodyInput {
    /// Seed the body from a built-in policy template.
    Template {
        /// The built-in policy template to seed from.
        template: PolicyTemplate,
    },
    /// An inline structured policy body consumed by the engine.
    Inline {
        /// The client-authored policy body.
        definition: Box<PolicyDraftInput>,
    },
}

impl PolicyBodyInput {
    /// Resolves the body source into a concrete policy definition with a fresh
    /// `id`. An inline body carries no template origin; a template body keeps the
    /// template's own origin.
    #[must_use]
    pub fn into_definition(self) -> Policy {
        match self {
            PolicyBodyInput::Inline { definition } => definition.into_definition(None),
            PolicyBodyInput::Template { template } => Policy {
                id: Uuid::now_v7(),
                ..template.build().policy
            },
        }
    }
}

/// A derived policy: a bare list of built-in labels, each removed. Minted (or
/// reused) when a detection runs over labels rather than named policies.
#[derive(Clone)]
pub struct DerivedPolicyInput {
    /// The built-in labels to detect and remove.
    pub labels: Vec<String>,
}

impl DerivedPolicyInput {
    /// The content-address hash of the label set.
    ///
    /// A derived policy is identified by its label set (every derived policy
    /// removes, so the action is not part of the identity). Labels are sorted and
    /// deduplicated so the same set hashes equally regardless of order, letting an
    /// identical derived policy be reused rather than duplicated. The minted
    /// definition `id` is not part of the hash.
    pub fn content_hash(&self) -> Vec<u8> {
        let mut ids: Vec<&str> = self.labels.iter().map(String::as_str).collect();
        ids.sort_unstable();
        ids.dedup();

        let mut hasher = Sha256::new();
        for id in ids {
            hasher.update(u32::try_from(id.len()).unwrap_or(u32::MAX).to_le_bytes());
            hasher.update(id.as_bytes());
        }
        hasher.finalize().to_vec()
    }

    /// Builds the degenerate definition a derived policy holds — one scope over
    /// the labels, each removed — under `name`, with a fresh `id`.
    pub fn into_definition(self, name: &str) -> Policy {
        let refs = self.labels.into_iter().map(LabelRef::new);
        Policy {
            id: Uuid::now_v7(),
            name: name.into(),
            description: None,
            template: None,
            scopes: vec![LabelScope::new(name.to_owned(), refs)],
            rules: Vec::new(),
            fallback: Some(ModalityRedactions::text(TextRedaction::Remove)),
        }
    }
}

/// Input for creating an authored policy: the body plus optional identity
/// overrides.
pub struct CreatePolicyInput {
    /// Optional display name override.
    pub display_name: Option<String>,
    /// Optional description override.
    pub description: Option<String>,
    /// The source of the policy body.
    pub body: PolicyBodyInput,
}

/// Input for updating an authored policy. Replacing the definition replaces the
/// whole body; the template origin is server-owned and preserved.
pub struct UpdatePolicyInput {
    /// New display name.
    pub display_name: Option<String>,
    /// New description (outer `Some` sets it, inner `None` clears it).
    pub description: Option<Option<String>>,
    /// New policy body, replacing the stored definition.
    pub definition: Option<PolicyDraftInput>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_policy_removes_the_picked_labels() {
        let body = DerivedPolicyInput {
            labels: vec!["person_name".to_owned(), "email_address".to_owned()],
        };

        let definition = body.into_definition("d-abc123");
        assert_eq!(&*definition.name, "d-abc123");
        assert!(definition.rules.is_empty());
        assert_eq!(definition.scopes.len(), 1);
        let labels: Vec<&str> = definition.scopes[0]
            .labels
            .iter()
            .map(LabelRef::as_str)
            .collect();
        assert_eq!(labels, vec!["person_name", "email_address"]);
        assert!(matches!(
            definition.fallback.as_ref().and_then(|f| f.text.as_ref()),
            Some(TextRedaction::Remove)
        ));
    }

    #[test]
    fn derived_hash_is_order_insensitive() {
        let a = DerivedPolicyInput {
            labels: vec!["person_name".to_owned(), "email_address".to_owned()],
        };
        let b = DerivedPolicyInput {
            labels: vec!["email_address".to_owned(), "person_name".to_owned()],
        };
        assert_eq!(a.content_hash(), b.content_hash());

        let different = DerivedPolicyInput {
            labels: vec!["ip_address".to_owned()],
        };
        assert_ne!(a.content_hash(), different.content_hash());
    }
}
