//! Policy response types.

use elide_pipeline::governance::policy::Policy;
use jiff::Timestamp;
use nvisy_postgres::model::{WorkspacePolicy as WorkspacePolicyModel, WorkspacePolicyVersion};
use nvisy_postgres::types::{Handle, PolicyKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AccountRef, Page};
use crate::response::{ErrorKind, Result};

/// Response type for a workspace policy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePolicy {
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
    /// The structured policy body consumed by the engine (the current version).
    pub definition: Policy,
    /// The current version number of the policy's definition.
    pub version_number: i32,
    /// How the policy came to exist. A one-shot policy (minted from labels) is
    /// hidden from the list and not pipeline-attachable until promoted.
    pub kind: PolicyKind,
    /// When the policy was created.
    pub created_at: Timestamp,
    /// When the policy was last updated.
    pub updated_at: Timestamp,
}

/// Lightweight policy view for lists.
///
/// Carries only the metadata, without loading the policy body, so a page of
/// policies stays small. The full [`WorkspacePolicy`] (with its `definition`) is
/// returned by the single-policy endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePolicySummary {
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

impl WorkspacePolicySummary {
    /// Creates a summary from a database model and its creator. Does not load the
    /// definition.
    pub fn from_model(
        policy: WorkspacePolicyModel,
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
pub type PoliciesPage = Page<WorkspacePolicySummary>;

impl WorkspacePolicy {
    /// Creates a response from a policy and its current version, deserializing the
    /// version's plaintext definition.
    pub fn from_model(
        policy: WorkspacePolicyModel,
        version: WorkspacePolicyVersion,
        workspace_slug: Handle,
        created_by: AccountRef,
    ) -> Result<Self> {
        let definition = serde_json::from_value::<Policy>(version.definition).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Stored policy definition is malformed")
                .with_context(err.to_string())
        })?;

        Ok(Self {
            slug: policy.slug,
            workspace_slug,
            created_by,
            display_name: policy.display_name,
            description: policy.description,
            definition,
            version_number: version.version_number,
            kind: policy.kind,
            created_at: policy.created_at.into(),
            updated_at: policy.updated_at.into(),
        })
    }
}
