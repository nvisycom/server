//! Enrich transformer configuration - add metadata/descriptions to elements.

use serde::{Deserialize, Serialize};

use crate::provider::CompletionProviderParams;

/// Configuration for enriching elements with metadata/descriptions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnrichConfig {
    /// Completion provider parameters (includes credentials_id and model).
    #[serde(flatten)]
    pub provider: CompletionProviderParams,

    /// The enrichment task to perform.
    #[serde(flatten)]
    pub task: EnrichTask,

    /// Optional prompt override for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_prompt: Option<String>,
}

/// Tasks for adding metadata/descriptions to elements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "input_type", content = "task", rename_all = "snake_case")]
pub enum EnrichTask {
    /// Enrich table elements.
    Table(TableEnrichTask),
    /// Enrich image elements.
    Image(ImageEnrichTask),
}

/// Tasks for table enrichment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableEnrichTask {
    /// Generate a natural language description of the table.
    Description,
    /// Generate descriptions for each column.
    ColumnDescriptions,
}

/// Tasks for image enrichment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageEnrichTask {
    /// Generate a natural language description of the image.
    Description,
    /// Generate a detailed description (people, objects, text, colors, layout).
    DetailedDescription,
    /// Extract text from image using generative OCR.
    GenerativeOcr,
    /// Detect and list objects/entities in the image.
    ObjectDetection,
}
