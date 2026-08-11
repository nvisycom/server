//! Pipeline request types.
//!
//! This module provides request DTOs for pipeline management operations including
//! creation, updates, and filtering. All request types support JSON serialization
//! and validation.

use nvisy_engine::plan::ScopeParams;
use nvisy_postgres::model::{NewWorkspacePipeline, UpdateWorkspacePipeline as UpdatePipelineModel};
use nvisy_postgres::types::{Handle, PipelineStatus, RetentionOverride};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// A pipeline's detection + governance intent.
///
/// Holds what a pipeline author decides — the default scope and the policies to
/// apply. Recognition is entirely server-wide: the built-in pattern set plus the
/// deployment's NER/LLM lineups and enrichment backends live in the engine
/// config, not here. Stored as JSON in the pipeline's `definition` column but
/// validated against this schema at the API boundary.
///
/// The label catalog is not part of this: the policies own the label vocabulary,
/// and the engine derives the detection catalog from them at run time.
///
/// The split:
///
/// - `default_scope` — optional pipeline-wide scope a document may override.
/// - `policy_slugs` — references to the workspace's policies, resolved at run
///   time.
#[must_use]
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PipelineDefinition {
    /// Optional pipeline-wide scope (languages, jurisdictions, document labels).
    ///
    /// A document's own scope overrides this at detect time; absent here means
    /// the document must assert its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scope: Option<ScopeParams>,
    /// Slugs of workspace policies applied at redaction.
    ///
    /// Stored relationally in the `workspace_pipeline_policies` join table, not the JSON
    /// definition; surfaced here so the API exposes one coherent object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(length(max = 64))]
    pub policy_slugs: Vec<Handle>,
}

impl PipelineDefinition {
    /// Splits the definition into its stored parts: the engine config JSON (with
    /// the relational references removed) and the policy reference slugs.
    ///
    /// The references live in a join table, so they are stripped from the JSON to
    /// keep a single source of truth. Serialization failure is surfaced rather
    /// than swallowed so a bad config never gets silently persisted as empty.
    pub fn into_parts(mut self) -> serde_json::Result<(serde_json::Value, Vec<Handle>)> {
        let policy_slugs = std::mem::take(&mut self.policy_slugs);
        let config = serde_json::to_value(&self)?;
        Ok((config, policy_slugs))
    }

    /// Rebuilds a definition from stored config JSON and the reference slugs read
    /// back from the join table.
    ///
    /// Decoding failure is surfaced rather than swallowed: a stored config that
    /// does not match the schema is a server-side data error, not an empty
    /// config to return silently.
    pub fn from_parts(
        config: serde_json::Value,
        policy_slugs: Vec<Handle>,
    ) -> serde_json::Result<Self> {
        let mut definition: Self = serde_json::from_value(config)?;
        definition.policy_slugs = policy_slugs;
        Ok(definition)
    }
}

/// Request payload for creating a new pipeline.
///
/// Creates a new pipeline with the specified name and optional description.
/// The definition can be added later via update.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreatePipeline {
    /// Pipeline display name (2-128 characters).
    #[validate(length(min = 2, max = 128))]
    pub display_name: String,
    /// URL slug, unique within the workspace and immutable after creation.
    pub slug: Handle,
    /// Optional description of the pipeline (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// Optional detection + redaction configuration. Defaults to an empty
    /// definition that can be filled in via update.
    #[validate(nested)]
    pub definition: Option<PipelineDefinition>,
    /// Optional per-scope data-retention override for this pipeline. Each unset
    /// scope inherits the workspace retention.
    pub retention: Option<RetentionOverride>,
}

/// A pipeline's reference slugs, split out to be resolved to ids and written to
/// the join table after the pipeline row exists.
#[derive(Debug, Default, Clone)]
pub struct PipelineReferences {
    /// Slugs of the policies the pipeline references.
    pub policy_slugs: Vec<Handle>,
}

impl CreatePipeline {
    /// Splits this request into the pipeline model and its reference ids.
    ///
    /// The stored model carries only the engine config JSON; the policy
    /// references are returned separately for the caller to persist into the
    /// join table (`None` when no definition was supplied).
    ///
    /// # Arguments
    ///
    /// * `workspace_id` - The ID of the workspace this pipeline belongs to.
    /// * `account_id` - The ID of the account creating the pipeline.
    pub fn into_parts(
        self,
        workspace_id: Uuid,
        account_id: Uuid,
    ) -> serde_json::Result<(NewWorkspacePipeline, PipelineReferences)> {
        let (definition, references) = split_definition(self.definition)?;
        let metadata = self
            .retention
            .map(|retention| retention.to_pipeline_metadata())
            .transpose()?;
        let model = NewWorkspacePipeline {
            workspace_id,
            account_id,
            slug: self.slug,
            display_name: self.display_name,
            description: self.description,
            status: None,
            definition: Some(definition),
            metadata,
            schedule_cron: None,
            schedule_tz: None,
            next_run_at: None,
        };
        Ok((model, references))
    }
}

/// Splits an optional definition into its stored JSON config and reference ids.
///
/// A missing definition stores the empty-config default (the `definition` column
/// is `NOT NULL`) and no references.
fn split_definition(
    definition: Option<PipelineDefinition>,
) -> serde_json::Result<(serde_json::Value, PipelineReferences)> {
    let (config, policy_slugs) = definition.unwrap_or_default().into_parts()?;
    let references = PipelineReferences { policy_slugs };
    Ok((config, references))
}

/// Request payload to update an existing pipeline.
///
/// All fields are optional; only provided fields will be updated. Supplying a
/// `definition` replaces the whole detection + redaction configuration.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePipeline {
    /// New display name for the pipeline (2-128 characters).
    #[validate(length(min = 2, max = 128))]
    pub display_name: Option<String>,
    /// New description for the pipeline (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// New status for the pipeline.
    pub status: Option<PipelineStatus>,
    /// New detection + redaction configuration (replaces the whole definition).
    #[validate(nested)]
    pub definition: Option<PipelineDefinition>,
    /// Replacement per-scope data-retention override. When omitted, the
    /// pipeline's retention override is left unchanged.
    pub retention: Option<RetentionOverride>,
}

impl UpdatePipeline {
    /// Splits this request into the update model, its reference ids, and the
    /// retention override (when the request set one).
    ///
    /// A missing `definition` leaves both the config column and the reference
    /// join table untouched (partial update); a present one replaces both, so
    /// the references are returned only in that case. The returned override lets
    /// the handler recompute `expires_at` on the pipeline's existing files when
    /// its retention changed.
    pub fn into_parts(
        self,
    ) -> serde_json::Result<(
        UpdatePipelineModel,
        Option<PipelineReferences>,
        Option<RetentionOverride>,
    )> {
        let (definition, references) = match self.definition {
            Some(definition) => {
                let (config, policy_slugs) = definition.into_parts()?;
                (Some(config), Some(PipelineReferences { policy_slugs }))
            }
            None => (None, None),
        };
        let metadata = self
            .retention
            .map(|retention| retention.to_pipeline_metadata())
            .transpose()?;
        let model = UpdatePipelineModel {
            display_name: self.display_name,
            description: self.description.map(Some),
            status: self.status,
            definition,
            metadata,
            ..Default::default()
        };
        Ok((model, references, self.retention))
    }
}

/// Query parameters for filtering pipelines.
#[must_use]
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct PipelineFilter {
    /// Filter by pipeline status.
    pub status: Option<PipelineStatus>,
    /// Search by pipeline name (trigram similarity).
    #[validate(length(max = 100))]
    pub search: Option<String>,
}
