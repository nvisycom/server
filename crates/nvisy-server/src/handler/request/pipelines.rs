//! Pipeline request types.
//!
//! This module provides request DTOs for pipeline management operations including
//! creation, updates, and filtering. All request types support JSON serialization
//! and validation.

use nvisy_postgres::model::{NewPipeline, UpdatePipeline as UpdatePipelineModel};
use nvisy_postgres::types::PipelineStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Request payload for creating a new pipeline.
///
/// Creates a new pipeline with the specified configuration. The creator is
/// automatically set as the owner of the pipeline.
#[must_use]
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreatePipeline {
    /// Pipeline name (3-100 characters).
    #[validate(length(min = 3, max = 100))]
    pub name: String,
    /// Optional description of the pipeline (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
    /// Pipeline definition containing steps and configuration.
    pub definition: Option<serde_json::Value>,
    /// Extended metadata for the pipeline.
    pub metadata: Option<serde_json::Value>,
}

impl CreatePipeline {
    /// Converts this request into a [`NewPipeline`] model for database insertion.
    ///
    /// # Arguments
    ///
    /// * `workspace_id` - The ID of the workspace this pipeline belongs to.
    /// * `account_id` - The ID of the account creating the pipeline.
    #[inline]
    pub fn into_model(self, workspace_id: Uuid, account_id: Uuid) -> NewPipeline {
        NewPipeline {
            workspace_id,
            account_id,
            name: self.name,
            description: self.description,
            definition: self.definition,
            metadata: self.metadata,
            ..Default::default()
        }
    }
}

/// Request payload to update an existing pipeline.
///
/// All fields are optional; only provided fields will be updated.
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
    /// New definition for the pipeline.
    pub definition: Option<serde_json::Value>,
    /// New metadata for the pipeline.
    pub metadata: Option<serde_json::Value>,
}

impl UpdatePipeline {
    /// Converts this request into an [`UpdatePipelineModel`] for database update.
    pub fn into_model(self) -> UpdatePipelineModel {
        UpdatePipelineModel {
            name: self.name,
            description: self.description.map(Some),
            status: self.status,
            definition: self.definition,
            metadata: self.metadata,
            ..Default::default()
        }
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
