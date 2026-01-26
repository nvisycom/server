//! Extract transform definition.

use nvisy_core::Provider;
use nvisy_rig::provider::{CompletionCredentials, CompletionModel, CompletionProvider};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Extract transform for extracting structured data or converting formats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extract {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,

    /// Completion model to use.
    #[serde(flatten)]
    pub model: CompletionModel,

    /// The extraction task to perform.
    #[serde(flatten)]
    pub task: ExtractTask,

    /// Optional prompt override for the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_prompt: Option<String>,
}

impl Extract {
    /// Creates a completion provider from these parameters and credentials.
    pub async fn into_provider(
        self,
        credentials: CompletionCredentials,
    ) -> Result<CompletionProvider> {
        CompletionProvider::connect(self.model, credentials)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
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
