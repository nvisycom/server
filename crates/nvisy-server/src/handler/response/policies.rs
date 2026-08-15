//! Policy response types.

use elide_pipeline::policy::PolicyDefinition;
use jiff::Timestamp;
use nvisy_postgres::model::WorkspacePolicy;
use nvisy_postgres::types::Handle;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AccountRef, Page};
use crate::service::CryptoService;

/// Response type for a workspace policy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    /// URL slug of the policy, unique within its workspace.
    pub slug: Handle,
    /// Handle of the workspace this policy belongs to.
    pub workspace_slug: Handle,
    /// Account that created this policy.
    pub created_by: AccountRef,
    /// Human-readable policy display name.
    pub display_name: String,
    /// Policy description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The structured policy body consumed by the engine.
    pub definition: PolicyDefinition,
    /// When the policy was created.
    pub created_at: Timestamp,
    /// When the policy was last updated.
    pub updated_at: Timestamp,
}

/// Lightweight policy view for lists.
///
/// Carries only the metadata available without decrypting the policy body, so a
/// page of policies costs no per-item decryption. The full [`Policy`] (with its
/// `definition`) is returned by the single-policy endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicySummary {
    /// URL slug of the policy, unique within its workspace.
    pub slug: Handle,
    /// Handle of the workspace this policy belongs to.
    pub workspace_slug: Handle,
    /// Account that created this policy.
    pub created_by: AccountRef,
    /// Human-readable policy display name.
    pub display_name: String,
    /// Policy description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When the policy was created.
    pub created_at: Timestamp,
    /// When the policy was last updated.
    pub updated_at: Timestamp,
}

impl PolicySummary {
    /// Creates a summary from a database model and its creator. Does not touch
    /// the encrypted definition.
    pub fn from_model(
        policy: WorkspacePolicy,
        workspace_slug: Handle,
        created_by: AccountRef,
    ) -> Self {
        Self {
            slug: policy.slug,
            workspace_slug,
            created_by,
            display_name: policy.display_name,
            description: policy.description,
            created_at: policy.created_at.into(),
            updated_at: policy.updated_at.into(),
        }
    }
}

/// Paginated list of policy summaries.
pub type PoliciesPage = Page<PolicySummary>;

impl Policy {
    /// Creates a response from a database model and its created_by, decrypting the
    /// definition.
    pub fn from_model(
        policy: WorkspacePolicy,
        workspace_slug: Handle,
        created_by: AccountRef,
        crypto: &CryptoService,
    ) -> crate::handler::Result<Self> {
        let definition =
            crypto.decrypt_json::<PolicyDefinition>(policy.workspace_id, &policy.definition)?;

        Ok(Self {
            slug: policy.slug,
            workspace_slug,
            created_by,
            display_name: policy.display_name,
            description: policy.description,
            definition,
            created_at: policy.created_at.into(),
            updated_at: policy.updated_at.into(),
        })
    }
}
