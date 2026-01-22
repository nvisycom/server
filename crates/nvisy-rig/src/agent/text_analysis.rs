//! Text analysis agent for extracting structured information.

use std::collections::HashMap;

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;
use serde::{Deserialize, Serialize};

use crate::provider::CompletionProvider;
use crate::{Error, Result};

/// A named entity extracted from text.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    /// The matched category labels.
    pub labels: Vec<String>,
    /// Confidence scores for each label (0.0 to 1.0).
    pub confidence: HashMap<String, f64>,
}

/// Sentiment analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// The first entity in the relationship.
    pub subject: String,
    /// The type of relationship.
    pub predicate: String,
    /// The second entity in the relationship.
    pub object: String,
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
pub struct TextAnalysisAgent {
    agent: Agent<CompletionProvider>,
}

impl TextAnalysisAgent {
    /// Creates a new text analysis agent with the given completion provider.
    pub fn new(provider: CompletionProvider) -> Self {
        let agent = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .build();
        Self { agent }
    }

    /// Extracts named entities from text.
    pub async fn extract_entities(&self, text: &str) -> Result<Vec<Entity>> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_ENTITIES, text);
        let response = self.agent.prompt(&prompt).await?;
        parse_json(&response)
    }

    /// Extracts keywords from text.
    pub async fn extract_keywords(&self, text: &str) -> Result<Vec<String>> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_KEYWORDS, text);
        let response = self.agent.prompt(&prompt).await?;
        parse_json(&response)
    }

    /// Classifies text into provided categories.
    pub async fn classify(&self, text: &str, labels: &[String]) -> Result<Classification> {
        let labels_str = labels.join(", ");
        let base_prompt = PROMPT_CLASSIFY.replace("{}", &labels_str);
        let prompt = format!("{}\n\nText:\n{}", base_prompt, text);
        let response = self.agent.prompt(&prompt).await?;
        parse_json(&response)
    }

    /// Analyzes sentiment of text.
    pub async fn analyze_sentiment(&self, text: &str) -> Result<Sentiment> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_ANALYZE_SENTIMENT, text);
        let response = self.agent.prompt(&prompt).await?;
        parse_json(&response)
    }

    /// Extracts relationships between entities in text.
    pub async fn extract_relationships(&self, text: &str) -> Result<Vec<Relationship>> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_RELATIONSHIPS, text);
        let response = self.agent.prompt(&prompt).await?;
        parse_json(&response)
    }
}

/// Parses JSON from LLM response, handling markdown code blocks.
fn parse_json<T: serde::de::DeserializeOwned>(response: &str) -> Result<T> {
    // Try to extract JSON from markdown code block if present
    let json_str = if response.contains("```json") {
        response
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .map(|s| s.trim())
            .unwrap_or(response.trim())
    } else if response.contains("```") {
        response
            .split("```")
            .nth(1)
            .map(|s| s.trim())
            .unwrap_or(response.trim())
    } else {
        response.trim()
    };

    serde_json::from_str(json_str).map_err(|e| Error::parse(format!("invalid JSON: {e}")))
}
