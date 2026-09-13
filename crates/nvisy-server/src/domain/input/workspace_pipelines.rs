//! Pipeline service inputs.

use elide_pipeline::provider::DocumentContext;
use nvisy_postgres::model::{NewWorkspacePipeline, UpdateWorkspacePipeline as UpdatePipelineModel};
use nvisy_postgres::types::{Json, PipelineMetadata, PipelineStatus, RetentionOverride};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A pipeline's detection + governance intent.
///
/// Holds what a pipeline author decides — the default scope and the policies to
/// apply. Recognition is server-wide (the engine config owns the pattern set and
/// the NER/LLM lineups), so it is not represented here. Stored as JSON in the
/// pipeline's `definition` column, with the policy references split out into the
/// `workspace_pipeline_policies` join table.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct PipelineDefinitionInput {
    /// Optional pipeline-wide scope a document may override at detect time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scope: Option<DocumentContext>,
    /// Ids of workspace policies applied at redaction, stored relationally.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policy_ids: Vec<Uuid>,
}

impl PipelineDefinitionInput {
    /// Splits the definition into its stored parts: the engine config JSON (with
    /// the relational references removed) and the policy reference ids.
    ///
    /// The references live in a join table, so they are stripped from the JSON to
    /// keep a single source of truth. Serialization failure is surfaced rather
    /// than swallowed so a bad config never gets silently persisted as empty.
    pub fn into_parts(mut self) -> serde_json::Result<(serde_json::Value, Vec<Uuid>)> {
        let policy_ids = std::mem::take(&mut self.policy_ids);
        let config = serde_json::to_value(&self)?;
        Ok((config, policy_ids))
    }

    /// Rebuilds a definition from stored config JSON and the reference ids read
    /// back from the join table.
    ///
    /// Decoding failure is surfaced rather than swallowed: a stored config that
    /// does not match the schema is a server-side data error, not an empty
    /// config to return silently.
    pub fn from_parts(
        config: serde_json::Value,
        policy_ids: Vec<Uuid>,
    ) -> serde_json::Result<Self> {
        let mut definition: Self = serde_json::from_value(config)?;
        definition.policy_ids = policy_ids;
        Ok(definition)
    }
}

/// A pipeline's reference ids, split out to be validated and written to the join
/// table after the pipeline row exists.
#[derive(Default, Clone)]
pub struct PipelineReferences {
    /// Ids of the policies the pipeline references.
    pub policy_ids: Vec<Uuid>,
}

/// Input for creating a pipeline: the body plus its optional definition.
pub struct CreatePipelineInput {
    /// Pipeline display name.
    pub display_name: String,
    /// Optional description of the pipeline.
    pub description: Option<String>,
    /// Optional detection + redaction configuration.
    pub definition: Option<PipelineDefinitionInput>,
    /// Optional lifecycle status. Defaults to `draft` when unset.
    pub status: Option<PipelineStatus>,
    /// Optional per-scope data-retention override for this pipeline.
    pub retention: Option<RetentionOverride>,
}

impl CreatePipelineInput {
    /// Splits this input into the pipeline model and its reference ids.
    ///
    /// The stored model carries only the engine config JSON; the policy
    /// references are returned separately for the caller to persist into the
    /// join table.
    pub fn into_parts(
        self,
        workspace_id: Uuid,
        account_id: Uuid,
    ) -> serde_json::Result<(NewWorkspacePipeline, PipelineReferences)> {
        let (config, policy_ids) = self.definition.unwrap_or_default().into_parts()?;
        let references = PipelineReferences { policy_ids };
        let metadata = self.retention.map(|retention| {
            Json::encode(&PipelineMetadata {
                retention: Some(retention),
                ..Default::default()
            })
        });
        let model = NewWorkspacePipeline {
            workspace_id,
            account_id,
            display_name: self.display_name,
            description: self.description,
            status: self.status,
            definition: Some(config),
            metadata,
        };
        Ok((model, references))
    }
}

/// Input for updating a pipeline. Only provided fields change; a supplied
/// definition replaces the whole detection + redaction configuration.
pub struct UpdatePipelineInput {
    /// New display name for the pipeline.
    pub display_name: Option<String>,
    /// New description for the pipeline.
    pub description: Option<String>,
    /// New status for the pipeline.
    pub status: Option<PipelineStatus>,
    /// New detection + redaction configuration (replaces the whole definition).
    pub definition: Option<PipelineDefinitionInput>,
    /// Replacement per-scope data-retention override.
    pub retention: Option<RetentionOverride>,
}

impl UpdatePipelineInput {
    /// Splits this input into the update model and its reference ids.
    ///
    /// A missing `definition` leaves both the config column and the reference
    /// join table untouched (partial update); a present one replaces both, so
    /// the references are returned only in that case. A supplied retention
    /// override is merged into the metadata column (preserving other fields).
    pub fn into_parts(
        self,
        current_metadata: PipelineMetadata,
    ) -> serde_json::Result<(UpdatePipelineModel, Option<PipelineReferences>)> {
        let (definition, references) = match self.definition {
            Some(definition) => {
                let (config, policy_ids) = definition.into_parts()?;
                (Some(config), Some(PipelineReferences { policy_ids }))
            }
            None => (None, None),
        };
        let metadata = self.retention.map(|retention| {
            Json::encode(&PipelineMetadata {
                retention: Some(retention),
                tags: current_metadata.tags,
            })
        });
        let model = UpdatePipelineModel {
            display_name: self.display_name,
            description: self.description.map(Some),
            status: self.status,
            definition,
            metadata,
            ..Default::default()
        };
        Ok((model, references))
    }
}
