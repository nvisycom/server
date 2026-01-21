//! Type-safe completion model references.

use serde::{Deserialize, Serialize};

/// Reference to a completion/chat model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "model", rename_all = "snake_case")]
pub enum CompletionModel {
    /// OpenAI completion models.
    OpenAi(OpenAiCompletionModel),
    /// Anthropic models.
    Anthropic(AnthropicModel),
    /// Cohere completion models.
    Cohere(CohereCompletionModel),
    /// Google Gemini completion models.
    Gemini(GeminiCompletionModel),
    /// Perplexity models.
    Perplexity(PerplexityModel),
    /// Ollama local models (model name as string).
    Ollama(String),
}

/// OpenAI completion models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenAiCompletionModel {
    /// GPT-4o (multimodal flagship)
    Gpt4o,
    /// GPT-4o mini (fast, affordable)
    Gpt4oMini,
    /// GPT-4 Turbo
    Gpt4Turbo,
    /// o1 (reasoning)
    O1,
    /// o1 mini (fast reasoning)
    O1Mini,
    /// o3 mini (latest reasoning)
    O3Mini,
}

impl OpenAiCompletionModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gpt4o => "gpt-4o",
            Self::Gpt4oMini => "gpt-4o-mini",
            Self::Gpt4Turbo => "gpt-4-turbo",
            Self::O1 => "o1",
            Self::O1Mini => "o1-mini",
            Self::O3Mini => "o3-mini",
        }
    }
}

/// Anthropic models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnthropicModel {
    /// Claude Opus 4 (most capable)
    ClaudeOpus4,
    /// Claude Sonnet 4 (balanced)
    ClaudeSonnet4,
    /// Claude Haiku 3.5 (fast)
    ClaudeHaiku35,
}

impl AnthropicModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeOpus4 => "claude-opus-4-20250514",
            Self::ClaudeSonnet4 => "claude-sonnet-4-20250514",
            Self::ClaudeHaiku35 => "claude-3-5-haiku-20241022",
        }
    }
}

/// Cohere completion models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CohereCompletionModel {
    /// Command R+ (most capable)
    CommandRPlus,
    /// Command R (balanced)
    CommandR,
    /// Command (legacy)
    Command,
}

impl CohereCompletionModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CommandRPlus => "command-r-plus",
            Self::CommandR => "command-r",
            Self::Command => "command",
        }
    }
}

/// Google Gemini completion models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeminiCompletionModel {
    /// Gemini 2.0 Flash (fast, multimodal)
    Gemini20Flash,
    /// Gemini 2.0 Flash Thinking (reasoning)
    Gemini20FlashThinking,
    /// Gemini 1.5 Pro (long context)
    Gemini15Pro,
    /// Gemini 1.5 Flash (fast)
    Gemini15Flash,
}

impl GeminiCompletionModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gemini20Flash => "gemini-2.0-flash",
            Self::Gemini20FlashThinking => "gemini-2.0-flash-thinking-exp",
            Self::Gemini15Pro => "gemini-1.5-pro",
            Self::Gemini15Flash => "gemini-1.5-flash",
        }
    }
}

/// Perplexity models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PerplexityModel {
    /// Sonar (online, search-augmented)
    Sonar,
    /// Sonar Pro (online, more capable)
    SonarPro,
    /// Sonar Reasoning (online, reasoning)
    SonarReasoning,
}

impl PerplexityModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sonar => "sonar",
            Self::SonarPro => "sonar-pro",
            Self::SonarReasoning => "sonar-reasoning",
        }
    }
}

impl CompletionModel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenAi(m) => m.as_str(),
            Self::Anthropic(m) => m.as_str(),
            Self::Cohere(m) => m.as_str(),
            Self::Gemini(m) => m.as_str(),
            Self::Perplexity(m) => m.as_str(),
            Self::Ollama(m) => m.as_str(),
        }
    }
}
