//! Completion provider abstraction.

use std::sync::Arc;

use nvisy_core::Provider;
use rig::completion::{AssistantContent, CompletionError, CompletionModel as RigCompletionModel};
use rig::message::Message;
use rig::one_or_many::OneOrMany;
use rig::prelude::CompletionClient;
use rig::providers::{anthropic, cohere, gemini, openai, perplexity};

use super::super::credentials::Credentials;
use super::model::{AnthropicModel, CompletionModel};
use crate::Error;

/// Completion provider that wraps different rig completion model implementations.
///
/// This is a cheaply cloneable wrapper around an `Arc<CompletionService>`.
#[derive(Clone)]
pub struct CompletionProvider(Arc<CompletionService>);

pub(crate) enum CompletionService {
    OpenAi {
        model: openai::CompletionModel,
        model_name: String,
    },
    Anthropic {
        model: anthropic::completion::CompletionModel,
        model_name: String,
    },
    Cohere {
        model: cohere::CompletionModel,
        model_name: String,
    },
    Gemini {
        model: gemini::completion::CompletionModel,
        model_name: String,
    },
    Perplexity {
        model: perplexity::CompletionModel,
        model_name: String,
    },
}

#[async_trait::async_trait]
impl Provider for CompletionProvider {
    type Credentials = Credentials;
    type Params = CompletionModel;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = match (credentials, params) {
            (Credentials::OpenAi(c), CompletionModel::OpenAi(m)) => {
                let client = openai::Client::new(&c.api_key)
                    .map_err(|e| Error::provider("openai", e.to_string()))?
                    .completions_api();
                CompletionService::OpenAi {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (Credentials::Anthropic(c), CompletionModel::Anthropic(m)) => {
                let client = anthropic::Client::new(&c.api_key)
                    .map_err(|e| Error::provider("anthropic", e.to_string()))?;
                CompletionService::Anthropic {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (Credentials::Cohere(c), CompletionModel::Cohere(m)) => {
                let client = cohere::Client::new(&c.api_key)
                    .map_err(|e| Error::provider("cohere", e.to_string()))?;
                CompletionService::Cohere {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (Credentials::Gemini(c), CompletionModel::Gemini(m)) => {
                let client = gemini::Client::new(&c.api_key)
                    .map_err(|e| Error::provider("gemini", e.to_string()))?;
                CompletionService::Gemini {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (Credentials::Perplexity(c), CompletionModel::Perplexity(m)) => {
                let client = perplexity::Client::new(&c.api_key)
                    .map_err(|e| Error::provider("perplexity", e.to_string()))?;
                CompletionService::Perplexity {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            _ => return Err(Error::config("mismatched credentials and model provider").into()),
        };
        Ok(Self(Arc::new(inner)))
    }
}

impl CompletionProvider {
    /// Returns a reference to the inner provider.
    pub(crate) fn inner(&self) -> &CompletionService {
        &self.0
    }

    /// Creates an Anthropic completion provider with a specific model.
    pub fn anthropic(api_key: &str, model: AnthropicModel) -> nvisy_core::Result<Self> {
        let client = anthropic::Client::new(api_key)
            .map_err(|e| Error::provider("anthropic", e.to_string()))?;
        Ok(Self(Arc::new(CompletionService::Anthropic {
            model: client.completion_model(model.as_ref()),
            model_name: model.as_ref().to_string(),
        })))
    }

    /// Returns the model name.
    pub fn model_name(&self) -> &str {
        match self.0.as_ref() {
            CompletionService::OpenAi { model_name, .. } => model_name,
            CompletionService::Anthropic { model_name, .. } => model_name,
            CompletionService::Cohere { model_name, .. } => model_name,
            CompletionService::Gemini { model_name, .. } => model_name,
            CompletionService::Perplexity { model_name, .. } => model_name,
        }
    }

    /// Returns the provider name.
    pub fn provider_name(&self) -> &'static str {
        match self.0.as_ref() {
            CompletionService::OpenAi { .. } => "openai",
            CompletionService::Anthropic { .. } => "anthropic",
            CompletionService::Cohere { .. } => "cohere",
            CompletionService::Gemini { .. } => "gemini",
            CompletionService::Perplexity { .. } => "perplexity",
        }
    }

    /// Sends a completion request with the given prompt and chat history.
    pub async fn complete(
        &self,
        prompt: &str,
        chat_history: Vec<Message>,
    ) -> nvisy_core::Result<String> {
        let model_name = self.model_name().to_string();
        let map_err = |e: CompletionError| {
            nvisy_core::Error::from(Error::provider(&model_name, e.to_string()))
        };

        match self.0.as_ref() {
            CompletionService::OpenAi { model, .. } => model
                .completion_request(prompt)
                .messages(chat_history)
                .send()
                .await
                .map(|r| extract_text_content(&r.choice))
                .map_err(map_err),
            CompletionService::Anthropic { model, .. } => model
                .completion_request(prompt)
                .messages(chat_history)
                .send()
                .await
                .map(|r| extract_text_content(&r.choice))
                .map_err(map_err),
            CompletionService::Cohere { model, .. } => model
                .completion_request(prompt)
                .messages(chat_history)
                .send()
                .await
                .map(|r| extract_text_content(&r.choice))
                .map_err(map_err),
            CompletionService::Gemini { model, .. } => model
                .completion_request(prompt)
                .messages(chat_history)
                .send()
                .await
                .map(|r| extract_text_content(&r.choice))
                .map_err(map_err),
            CompletionService::Perplexity { model, .. } => model
                .completion_request(prompt)
                .messages(chat_history)
                .send()
                .await
                .map(|r| extract_text_content(&r.choice))
                .map_err(map_err),
        }
    }
}

/// Extracts text content from assistant content choices.
fn extract_text_content(choice: &OneOrMany<AssistantContent>) -> String {
    choice
        .iter()
        .filter_map(|content| match content {
            AssistantContent::Text(text) => Some(text.text()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

impl std::fmt::Debug for CompletionProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.as_ref() {
            CompletionService::OpenAi { model_name, .. } => f
                .debug_struct("CompletionProvider::OpenAi")
                .field("model", model_name)
                .finish(),
            CompletionService::Anthropic { model_name, .. } => f
                .debug_struct("CompletionProvider::Anthropic")
                .field("model", model_name)
                .finish(),
            CompletionService::Cohere { model_name, .. } => f
                .debug_struct("CompletionProvider::Cohere")
                .field("model", model_name)
                .finish(),
            CompletionService::Gemini { model_name, .. } => f
                .debug_struct("CompletionProvider::Gemini")
                .field("model", model_name)
                .finish(),
            CompletionService::Perplexity { model_name, .. } => f
                .debug_struct("CompletionProvider::Perplexity")
                .field("model", model_name)
                .finish(),
        }
    }
}
