//! Extract transformer - extract structured data or convert formats.

use nvisy_dal::AnyDataValue;
use serde::{Deserialize, Serialize};

use super::Transform;
use crate::error::Result;
use crate::provider::{CompletionProviderParams, CredentialsRegistry};

/// Extract transformer for extracting structured data or converting formats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extract {
    /// Completion provider parameters (includes credentials_id and model).
    #[serde(flatten)]
    pub provider: CompletionProviderParams,

    /// The extraction task to perform.
    #[serde(flatten)]
    pub task: ExtractTask,

    /// Optional prompt override for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_prompt: Option<String>,
}

impl Transform for Extract {
    async fn transform(
        &self,
        input: Vec<AnyDataValue>,
        _registry: &CredentialsRegistry,
    ) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement extraction using completion provider
        // For now, pass through unchanged
        Ok(input)
    }
}

/// Tasks for extracting structured data or converting formats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "task_type", content = "task", rename_all = "snake_case")]
pub enum ExtractTask {
    /// Convert elements to different formats.
    Convert(ConvertTask),
    /// Analyze text to extract structured information.
    Analyze(AnalyzeTask),
}

/// Tasks for format conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "input_type",
    content = "convert_task",
    rename_all = "snake_case"
)]
pub enum ConvertTask {
    /// Convert table elements.
    Table(TableConvertTask),
    /// Convert text elements.
    Text(TextConvertTask),
}

/// Tasks for table conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableConvertTask {
    /// Convert table to HTML format.
    ToHtml,
    /// Convert table to Markdown format.
    ToMarkdown,
    /// Convert table to CSV format.
    ToCsv,
    /// Convert table to structured JSON.
    ToJson,
}

/// Tasks for text conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextConvertTask {
    /// Convert text to JSON format.
    ToJson,
    /// Convert text to structured JSON based on a schema.
    ToStructuredJson {
        /// JSON schema for the output structure.
        schema: String,
    },
}

/// Tasks for analyzing text to extract structured information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyzeTask {
    /// Extract named entities (people, places, organizations, dates, etc.).
    NamedEntityRecognition,
    /// Extract key terms and phrases.
    KeywordExtraction,
    /// Classify text into provided categories.
    Classification {
        /// Labels/categories for classification.
        labels: Vec<String>,
    },
    /// Analyze sentiment (positive, negative, neutral).
    SentimentAnalysis,
    /// Extract relationships between entities.
    RelationshipExtraction,
}
