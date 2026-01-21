//! Text analysis agent for extracting structured information.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;

use crate::Result;
use crate::provider::CompletionProvider;

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
    pub async fn extract_entities(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_ENTITIES, text);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Extracts keywords from text.
    pub async fn extract_keywords(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_KEYWORDS, text);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Classifies text into provided categories.
    pub async fn classify(&self, text: &str, labels: &[String]) -> Result<String> {
        let labels_str = labels.join(", ");
        let base_prompt = PROMPT_CLASSIFY.replace("{}", &labels_str);
        let prompt = format!("{}\n\nText:\n{}", base_prompt, text);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Analyzes sentiment of text.
    pub async fn analyze_sentiment(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_ANALYZE_SENTIMENT, text);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Extracts relationships between entities in text.
    pub async fn extract_relationships(&self, text: &str) -> Result<String> {
        let prompt = format!("{}\n\nText:\n{}", PROMPT_EXTRACT_RELATIONSHIPS, text);
        Ok(self.agent.prompt(&prompt).await?)
    }
}
