//! Pipeline request types.
//!
//! This module provides request DTOs for pipeline management operations including
//! creation, updates, and filtering. All request types support JSON serialization
//! and validation.

use nvisy_postgres::model::{NewWorkspacePipeline, UpdateWorkspacePipeline as UpdatePipelineModel};
use nvisy_postgres::types::{PipelineStatus, Slug};
use nvisy_schema::plan::{
    DeduplicationParams, EnricherParams, LabelCatalogParams, RecognizerParams, ScopeParams,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Reusable detection + redaction configuration for a pipeline.
///
/// The pipeline holds the "how to detect and redact" configuration the engine
/// consumes, minus the per-document assertions (which travel with a document at
/// detect time). Stored as JSON in the pipeline's `definition` column but
/// validated against this schema at the API boundary.
///
/// The split:
///
/// - `recognizers` / `enrichers` / `deduplication` / `label_catalog` — the
///   detection machinery, assembled into an engine `AnalyzerParams` per request.
/// - `default_scope` — optional pipeline-wide scope a document may override.
/// - `policy_slugs` / `context_slugs` — references to the workspace's policies
///   and contexts, resolved at run time.
#[must_use]
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PipelineDefinition {
    /// Recognizer lineup: pattern (incl. inline custom rules and
    /// dictionaries), plus the NER and LLM toggles.
    #[serde(default)]
    pub recognizers: RecognizerParams,
    /// Enrichers run before recognition: language, OCR, STT.
    #[serde(default)]
    pub enrichers: EnricherParams,
    /// Post-recognition deduplication pipeline.
    #[serde(default)]
    pub deduplication: DeduplicationParams,
    /// Entity-label catalog: which entity types the recognizers emit.
    ///
    /// Reusable across the pipeline's documents, so it lives here rather than in
    /// per-document scope.
    #[serde(default)]
    pub label_catalog: LabelCatalogParams,
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
    pub policy_slugs: Vec<Slug>,
    /// Slugs of workspace contexts supplied to detection.
    ///
    /// Stored relationally in the `workspace_pipeline_contexts` join table, not the JSON
    /// definition; surfaced here so the API exposes one coherent object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(length(max = 64))]
    pub context_slugs: Vec<Slug>,
}

impl PipelineDefinition {
    /// Splits the definition into its stored parts: the engine config JSON (with
    /// the relational references removed) and the policy / context reference slugs.
    ///
    /// The references live in join tables, so they are stripped from the JSON to
    /// keep a single source of truth. Serialization failure is surfaced rather
    /// than swallowed so a bad config never gets silently persisted as empty.
    pub fn into_parts(mut self) -> serde_json::Result<(serde_json::Value, Vec<Slug>, Vec<Slug>)> {
        let policy_slugs = std::mem::take(&mut self.policy_slugs);
        let context_slugs = std::mem::take(&mut self.context_slugs);
        let config = serde_json::to_value(&self)?;
        Ok((config, policy_slugs, context_slugs))
    }

    /// Rebuilds a definition from stored config JSON and the reference slugs read
    /// back from the join tables.
    ///
    /// Decoding failure is surfaced rather than swallowed: a stored config that
    /// does not match the schema is a server-side data error, not an empty
    /// config to return silently.
    pub fn from_parts(
        config: serde_json::Value,
        policy_slugs: Vec<Slug>,
        context_slugs: Vec<Slug>,
    ) -> serde_json::Result<Self> {
        let mut definition: Self = serde_json::from_value(config)?;
        definition.policy_slugs = policy_slugs;
        definition.context_slugs = context_slugs;
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
    /// Pipeline name (3-100 characters).
    #[validate(length(min = 3, max = 100))]
    pub name: String,
    /// URL slug, unique within the workspace and immutable after creation.
    pub slug: Slug,
    /// Optional description of the pipeline (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// Optional detection + redaction configuration. Defaults to an empty
    /// definition that can be filled in via update.
    #[validate(nested)]
    pub definition: Option<PipelineDefinition>,
}

/// A pipeline's reference slugs, split out to be resolved to ids and written to
/// the join tables after the pipeline row exists.
#[derive(Debug, Default, Clone)]
pub struct PipelineReferences {
    /// Slugs of the policies the pipeline references.
    pub policy_slugs: Vec<Slug>,
    /// Slugs of the contexts the pipeline references.
    pub context_slugs: Vec<Slug>,
}

impl CreatePipeline {
    /// Splits this request into the pipeline model and its reference ids.
    ///
    /// The stored model carries only the engine config JSON; the policy and
    /// context references are returned separately for the caller to persist into
    /// the join tables (`None` when no definition was supplied).
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
        let model = NewWorkspacePipeline {
            workspace_id,
            account_id,
            slug: self.slug,
            name: self.name,
            description: self.description,
            status: None,
            definition: Some(definition),
            metadata: None,
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
    let (config, policy_slugs, context_slugs) = definition.unwrap_or_default().into_parts()?;
    let references = PipelineReferences {
        policy_slugs,
        context_slugs,
    };
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
    /// New name for the pipeline (3-100 characters).
    #[validate(length(min = 3, max = 100))]
    pub name: Option<String>,
    /// New description for the pipeline (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// New status for the pipeline.
    pub status: Option<PipelineStatus>,
    /// New detection + redaction configuration (replaces the whole definition).
    #[validate(nested)]
    pub definition: Option<PipelineDefinition>,
}

impl UpdatePipeline {
    /// Splits this request into the update model and its reference ids.
    ///
    /// A missing `definition` leaves both the config column and the reference
    /// join tables untouched (partial update); a present one replaces both, so
    /// the references are returned only in that case.
    pub fn into_parts(
        self,
    ) -> serde_json::Result<(UpdatePipelineModel, Option<PipelineReferences>)> {
        let (definition, references) = match self.definition {
            Some(definition) => {
                let (config, policy_slugs, context_slugs) = definition.into_parts()?;
                (
                    Some(config),
                    Some(PipelineReferences {
                        policy_slugs,
                        context_slugs,
                    }),
                )
            }
            None => (None, None),
        };
        let model = UpdatePipelineModel {
            name: self.name,
            description: self.description.map(Some),
            status: self.status,
            definition,
            ..Default::default()
        };
        Ok((model, references))
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
