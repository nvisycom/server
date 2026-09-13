//! Pipeline request types.
//!
//! This module provides request DTOs for pipeline management operations including
//! creation, updates, and filtering. All request types support JSON serialization
//! and validation.

use elide_pipeline::provider::DocumentContext;
use garde::Validate;
use nvisy_postgres::types::{Handle, PipelineStatus, RetentionOverride};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::input::{CreatePipelineInput, PipelineDefinitionInput, UpdatePipelineInput};

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
#[garde(allow_unvalidated)]
pub struct PipelineDefinition {
    /// Optional pipeline-wide scope (languages, jurisdictions, document labels).
    ///
    /// A document's own scope overrides this at detect time; absent here means
    /// the document must assert its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scope: Option<DocumentContext>,
    /// Slugs of workspace policies applied at redaction.
    ///
    /// Stored relationally in the `workspace_pipeline_policies` join table, not the JSON
    /// definition; surfaced here so the API exposes one coherent object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(length(max = 64))]
    pub policy_slugs: Vec<Handle>,
}

impl PipelineDefinition {
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

impl From<PipelineDefinition> for PipelineDefinitionInput {
    fn from(definition: PipelineDefinition) -> Self {
        PipelineDefinitionInput {
            default_scope: definition.default_scope,
            policy_slugs: definition.policy_slugs,
        }
    }
}

/// Request payload for creating a new pipeline.
///
/// Creates a new pipeline with the specified name and optional description.
/// The definition can be added later via update.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct CreateWorkspacePipeline {
    /// Pipeline display name (2-128 characters).
    #[garde(length(chars, min = 2, max = 128))]
    pub display_name: String,
    /// URL slug, unique within the workspace and immutable after creation.
    pub slug: Handle,
    /// Optional description of the pipeline (max 500 characters).
    #[garde(length(chars, max = 500))]
    pub description: Option<String>,
    /// Optional detection + redaction configuration. Defaults to an empty
    /// definition that can be filled in via update.
    #[garde(dive)]
    pub definition: Option<PipelineDefinition>,
    /// Optional lifecycle status. Defaults to `draft`; pass `enabled` to create a
    /// pipeline ready to run without a follow-up update.
    pub status: Option<PipelineStatus>,
    /// Optional per-scope data-retention override for this pipeline. Each unset
    /// scope inherits the workspace retention.
    pub retention: Option<RetentionOverride>,
}

impl From<CreateWorkspacePipeline> for CreatePipelineInput {
    fn from(request: CreateWorkspacePipeline) -> Self {
        CreatePipelineInput {
            display_name: request.display_name,
            slug: request.slug,
            description: request.description,
            definition: request.definition.map(Into::into),
            status: request.status,
            retention: request.retention,
        }
    }
}

/// Request payload to update an existing pipeline.
///
/// All fields are optional; only provided fields will be updated. Supplying a
/// `definition` replaces the whole detection + redaction configuration.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct UpdateWorkspacePipeline {
    /// New display name for the pipeline (2-128 characters).
    #[garde(length(chars, min = 2, max = 128))]
    pub display_name: Option<String>,
    /// New description for the pipeline (max 500 characters).
    #[garde(length(chars, max = 500))]
    pub description: Option<String>,
    /// New status for the pipeline.
    pub status: Option<PipelineStatus>,
    /// New detection + redaction configuration (replaces the whole definition).
    #[garde(dive)]
    pub definition: Option<PipelineDefinition>,
    /// Replacement per-scope data-retention override. When omitted, the
    /// pipeline's retention override is left unchanged.
    pub retention: Option<RetentionOverride>,
}

impl From<UpdateWorkspacePipeline> for UpdatePipelineInput {
    fn from(request: UpdateWorkspacePipeline) -> Self {
        UpdatePipelineInput {
            display_name: request.display_name,
            description: request.description,
            status: request.status,
            definition: request.definition.map(Into::into),
            retention: request.retention,
        }
    }
}

/// Query parameters for filtering pipelines.
#[must_use]
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
#[garde(allow_unvalidated)]
pub struct WorkspacePipelineFilter {
    /// Filter by pipeline status.
    pub status: Option<PipelineStatus>,
    /// Search by pipeline name (trigram similarity).
    #[garde(length(chars, max = 100))]
    pub search: Option<String>,
}
