//! Text analysis agent for extracting structured information.

use std::collections::HashMap;

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::tool::{JsonResponse, JsonSchemaTool, ScratchpadTool};
use crate::Result;
use crate::provider::CompletionProvider;

/// A named entity extracted from text.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    /// The text of the entity.
    pub text: String,
    /// The type of entity (e.g., "person", "organization", "location").
    #[serde(rename = "type")]
    pub entity_type: String,
    /// The starting character index in the source text.
    #[serde(default)]
    pub start_index: Option<usize>,
}

/// Classification result with labels and confidence scores.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Classification {
    /// The matched category labels.
    pub labels: Vec<String>,
    /// Confidence scores for each label (0.0 to 1.0).
    pub confidence: HashMap<String, f64>,
}

/// Sentiment analysis result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Sentiment {
    /// The overall sentiment: "positive", "negative", "neutral", or "mixed".
    pub sentiment: String,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// Brief explanation of the sentiment.
    #[serde(default)]
    pub explanation: Option<String>,
}

/// A relationship between two entities.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Relationship {
    /// The first entity in the relationship.
    pub subject: String,
    /// The type of relationship.
    pub predicate: String,
    /// The second entity in the relationship.
    pub object: String,
}

/// Combined schema for text analysis outputs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextAnalysisOutput {
    /// Extracted entities.
    #[serde(default)]
    pub entities: Option<Vec<Entity>>,
    /// Extracted keywords.
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// Classification result.
    #[serde(default)]
    pub classification: Option<Classification>,
    /// Sentiment analysis result.
    #[serde(default)]
    pub sentiment: Option<Sentiment>,
    /// Extracted relationships.
    #[serde(default)]
    pub relationships: Option<Vec<Relationship>>,
}

const NAME: &str = "TextAnalysisAgent";
const DESCRIPTION: &str = "Agent for text analysis including entity extraction, keyword extraction, classification, and sentiment analysis";

const PREAMBLE: &str = "\
You are a text analysis assistant specialized in extracting structured information from text.
Your task is to identify entities, relationships, sentiment, and other structured data from unstructured text.
Be precise and comprehensive in your extractions.
Always output valid JSON format matching the requested structure.";

const PROMPT_EXTRACT_ENTITIES: &str = "\
Extract all named entities from the following text.
Identify: people, organizations, locations, dates, monetary values, and other notable entities.

Format as a JSON array with objects containing 'text', 'type', and 'start_index' fields.";

const PROMPT_EXTRACT_KEYWORDS: &str = "\
Extract the most important keywords and key phrases from the following text.
Return 5-15 keywords ordered by relevance.

Format as a JSON array of strings.";

const PROMPT_CLASSIFY: &str = "\
Classify the following text into one or more of these categories: {}

Format as a JSON object with 'labels' (array of matching categories) \
and 'confidence' (object mapping each label to a confidence score 0-1).";

const PROMPT_ANALYZE_SENTIMENT: &str = "\
Analyze the sentiment of the following text.

Format as a JSON object with:
- 'sentiment': one of 'positive', 'negative', 'neutral', or 'mixed'
- 'confidence': confidence score 0-1
- 'explanation': brief explanation of the sentiment";

const PROMPT_EXTRACT_RELATIONSHIPS: &str = "\
Extract relationships between entities in the following text.
Identify how people, organizations, and other entities are connected.

Format as a JSON array with objects containing:
- 'subject': the first entity
- 'predicate': the relationship type
- 'object': the second entity";

/// Agent for text analysis tasks.
///
/// Handles tasks that extract structured information from text:
/// - Named entity recognition (NER)
/// - Keyword extraction
/// - Classification
/// - Sentiment analysis
/// - Relationship extraction
///
/// When `with_tools` is enabled, the agent has access to:
/// - `ScratchpadTool` - For drafting and refining extractions
/// - `JsonSchemaTool` - For validating output against schemas
pub struct TextAnalysisAgent {
    agent: Agent<CompletionProvider>,
    model_name: String,
}

impl TextAnalysisAgent {
    /// Creates a new text analysis agent with the given completion provider.
    ///
    /// # Arguments
    /// * `provider` - The completion provider to use
    /// * `with_tools` - Whether to enable tool usage (scratchpad, schema validation)
    pub fn new(provider: CompletionProvider, with_tools: bool) -> Self {
        let model_name = provider.model_name().to_string();
        let builder = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE);

        let agent = if with_tools {
            builder
                .tool(ScratchpadTool::new())
                .tool(JsonSchemaTool::<TextAnalysisOutput>::new())
                .build()
        } else {
            builder.build()
        };

        Self { agent, model_name }
    }

    /// Extracts named entities from text.
    #[tracing::instrument(skip(self, text), fields(agent = NAME, model = %self.model_name, text_len = text.len()))]
    pub async fn extract_entities(&self, text: &str) -> Result<Vec<Entity>> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_ENTITIES, text);
        let response = self.agent.prompt(&prompt).await?;
        let entities: Vec<Entity> = JsonResponse::parse(&response)?;
        tracing::debug!(entity_count = entities.len(), "extract_entities completed");
        Ok(entities)
    }

    /// Extracts keywords from text.
    #[tracing::instrument(skip(self, text), fields(agent = NAME, model = %self.model_name, text_len = text.len()))]
    pub async fn extract_keywords(&self, text: &str) -> Result<Vec<String>> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_KEYWORDS, text);
        let response = self.agent.prompt(&prompt).await?;
        let keywords: Vec<String> = JsonResponse::parse(&response)?;
        tracing::debug!(keyword_count = keywords.len(), "extract_keywords completed");
        Ok(keywords)
    }

    /// Classifies text into provided categories.
    #[tracing::instrument(skip(self, text), fields(agent = NAME, model = %self.model_name, text_len = text.len(), label_count = labels.len()))]
    pub async fn classify(&self, text: &str, labels: &[String]) -> Result<Classification> {
        let labels_str = labels.join(", ");
        let base_prompt = PROMPT_CLASSIFY.replace("{}", &labels_str);
        let prompt = format!("{}\n\nText:\n{}", base_prompt, text);
        let response = self.agent.prompt(&prompt).await?;
        let classification: Classification = JsonResponse::parse(&response)?;
        tracing::debug!(matched_labels = ?classification.labels, "classify completed");
        Ok(classification)
    }

    /// Analyzes sentiment of text.
    #[tracing::instrument(skip(self, text), fields(agent = NAME, model = %self.model_name, text_len = text.len()))]
    pub async fn analyze_sentiment(&self, text: &str) -> Result<Sentiment> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_ANALYZE_SENTIMENT, text);
        let response = self.agent.prompt(&prompt).await?;
        let sentiment: Sentiment = JsonResponse::parse(&response)?;
        tracing::debug!(sentiment = %sentiment.sentiment, confidence = %sentiment.confidence, "analyze_sentiment completed");
        Ok(sentiment)
    }

    /// Extracts relationships between entities in text.
    #[tracing::instrument(skip(self, text), fields(agent = NAME, model = %self.model_name, text_len = text.len()))]
    pub async fn extract_relationships(&self, text: &str) -> Result<Vec<Relationship>> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_RELATIONSHIPS, text);
        let response = self.agent.prompt(&prompt).await?;
        let relationships: Vec<Relationship> = JsonResponse::parse(&response)?;
        tracing::debug!(
            relationship_count = relationships.len(),
            "extract_relationships completed"
        );
        Ok(relationships)
    }
}
