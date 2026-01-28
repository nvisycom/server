//! Type-safe completion model references.

use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};

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
}

/// OpenAI completion models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(AsRefStr, Display, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum OpenAiCompletionModel {
    /// GPT-4o (multimodal flagship)
    #[strum(serialize = "gpt-4o")]
    Gpt4o,
    /// GPT-4o mini (fast, affordable)
    #[strum(serialize = "gpt-4o-mini")]
    Gpt4oMini,
    /// GPT-4 Turbo
    #[strum(serialize = "gpt-4-turbo")]
    Gpt4Turbo,
    /// o1 (reasoning)
    #[strum(serialize = "o1")]
    O1,
    /// o1 mini (fast reasoning)
    #[strum(serialize = "o1-mini")]
    O1Mini,
    /// o3 mini (latest reasoning)
    #[strum(serialize = "o3-mini")]
    O3Mini,
}

/// Anthropic models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(AsRefStr, Display, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum AnthropicModel {
    /// Claude Opus 4 (most capable)
    #[strum(serialize = "claude-opus-4-20250514")]
    ClaudeOpus4,
    /// Claude Sonnet 4 (balanced)
    #[strum(serialize = "claude-sonnet-4-20250514")]
    ClaudeSonnet4,
    /// Claude Haiku 3.5 (fast)
    #[strum(serialize = "claude-3-5-haiku-20241022")]
    ClaudeHaiku35,
}

/// Cohere completion models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(AsRefStr, Display, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum CohereCompletionModel {
    /// Command R+ (most capable)
    #[strum(serialize = "command-r-plus")]
    CommandRPlus,
    /// Command R (balanced)
    #[strum(serialize = "command-r")]
    CommandR,
    /// Command (legacy)
    #[strum(serialize = "command")]
    Command,
}

/// Google Gemini completion models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(AsRefStr, Display, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum GeminiCompletionModel {
    /// Gemini 2.0 Flash (fast, multimodal)
    #[strum(serialize = "gemini-2.0-flash")]
    Gemini20Flash,
    /// Gemini 2.0 Flash Thinking (reasoning)
    #[strum(serialize = "gemini-2.0-flash-thinking-exp")]
    Gemini20FlashThinking,
    /// Gemini 1.5 Pro (long context)
    #[strum(serialize = "gemini-1.5-pro")]
    Gemini15Pro,
    /// Gemini 1.5 Flash (fast)
    #[strum(serialize = "gemini-1.5-flash")]
    Gemini15Flash,
}

/// Perplexity models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(AsRefStr, Display, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum PerplexityModel {
    /// Sonar (online, search-augmented)
    #[strum(serialize = "sonar")]
    Sonar,
    /// Sonar Pro (online, more capable)
    #[strum(serialize = "sonar-pro")]
    SonarPro,
    /// Sonar Reasoning (online, reasoning)
    #[strum(serialize = "sonar-reasoning")]
    SonarReasoning,
}

impl CompletionModel {
    /// Returns the model identifier string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenAi(m) => m.as_ref(),
            Self::Anthropic(m) => m.as_ref(),
            Self::Cohere(m) => m.as_ref(),
            Self::Gemini(m) => m.as_ref(),
            Self::Perplexity(m) => m.as_ref(),
        }
    }
}
