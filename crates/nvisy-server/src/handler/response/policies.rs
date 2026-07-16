//! Policy response types.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspacePolicy;
use nvisy_postgres::types::WorkspaceSlug;
use nvisy_schema::policy::Policy as SchemaPolicy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Page;
use crate::service::CryptoService;

/// Response type for a workspace policy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    /// Unique policy identifier.
    pub id: Uuid,
    /// Slug of the workspace this policy belongs to.
    pub workspace_slug: WorkspaceSlug,
    /// Account that created this policy.
    pub account_id: Uuid,
    /// Human-readable policy name.
    pub name: String,
    /// Policy description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semver of the policy body.
    pub version: String,
    /// The structured policy body consumed by the engine.
    pub definition: SchemaPolicy,
    /// When the policy was created.
    pub created_at: Timestamp,
    /// When the policy was last updated.
    pub updated_at: Timestamp,
}

/// Paginated list of policies.
pub type PoliciesPage = Page<Policy>;

impl Policy {
    /// Creates a response from a database model, decrypting the definition.
    pub fn from_model(
        policy: WorkspacePolicy,
        workspace_slug: WorkspaceSlug,
        crypto: &CryptoService,
    ) -> crate::handler::Result<Self> {
        let definition =
            crypto.decrypt_json::<SchemaPolicy>(policy.workspace_id, &policy.definition)?;

        Ok(Self {
            id: policy.id,
            workspace_slug,
            account_id: policy.account_id,
            name: policy.name,
            description: policy.description,
            version: policy.version,
            definition,
            created_at: policy.created_at.into(),
            updated_at: policy.updated_at.into(),
        })
    }
}
