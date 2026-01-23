//! Completion provider abstraction.

use std::sync::Arc;

use nvisy_core::IntoProvider;
#[cfg(feature = "ollama")]
use rig::client::Nothing;
use rig::completion::{AssistantContent, CompletionError, CompletionModel as RigCompletionModel};
use rig::message::Message;
use rig::one_or_many::OneOrMany;
use rig::prelude::CompletionClient;
#[cfg(feature = "ollama")]
use rig::providers::ollama;
use rig::providers::{anthropic, cohere, gemini, openai, perplexity};

use super::credentials::CompletionCredentials;
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
    #[cfg(feature = "ollama")]
    Ollama {
        client: ollama::Client,
        model_name: String,
    },
}

#[async_trait::async_trait]
impl IntoProvider for CompletionProvider {
    type Credentials = CompletionCredentials;
    type Params = CompletionModel;

    async fn create(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        let inner = match (credentials, params) {
            (CompletionCredentials::OpenAi { api_key }, CompletionModel::OpenAi(m)) => {
                let client = openai::Client::new(&api_key)
                    .map_err(|e| Error::provider("openai", e.to_string()))?
                    .completions_api();
                CompletionService::OpenAi {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (CompletionCredentials::Anthropic { api_key }, CompletionModel::Anthropic(m)) => {
                let client = anthropic::Client::new(&api_key)
                    .map_err(|e| Error::provider("anthropic", e.to_string()))?;
                CompletionService::Anthropic {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (CompletionCredentials::Cohere { api_key }, CompletionModel::Cohere(m)) => {
                let client = cohere::Client::new(&api_key)
                    .map_err(|e| Error::provider("cohere", e.to_string()))?;
                CompletionService::Cohere {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (CompletionCredentials::Gemini { api_key }, CompletionModel::Gemini(m)) => {
                let client = gemini::Client::new(&api_key)
                    .map_err(|e| Error::provider("gemini", e.to_string()))?;
                CompletionService::Gemini {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            (CompletionCredentials::Perplexity { api_key }, CompletionModel::Perplexity(m)) => {
                let client = perplexity::Client::new(&api_key)
                    .map_err(|e| Error::provider("perplexity", e.to_string()))?;
                CompletionService::Perplexity {
                    model: client.completion_model(m.as_ref()),
                    model_name: m.as_ref().to_string(),
                }
            }
            #[cfg(feature = "ollama")]
            (CompletionCredentials::Ollama { base_url }, CompletionModel::Ollama(model_name)) => {
                let client = ollama::Client::builder()
                    .api_key(Nothing)
                    .base_url(&base_url)
                    .build()
                    .map_err(|e| Error::provider("ollama", e.to_string()))?;
                CompletionService::Ollama {
                    client,
                    model_name: model_name.clone(),
                }
            }
            #[allow(unreachable_patterns)]
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

    /// Creates an Ollama completion provider (convenience for local development).
    #[cfg(feature = "ollama")]
    pub fn ollama(base_url: &str, model_name: &str) -> nvisy_core::Result<Self> {
        let client = ollama::Client::builder()
            .api_key(Nothing)
            .base_url(base_url)
            .build()
            .map_err(|e| Error::provider("ollama", e.to_string()))?;
        Ok(Self(Arc::new(CompletionService::Ollama {
            client,
            model_name: model_name.to_string(),
        })))
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
            #[cfg(feature = "ollama")]
            CompletionService::Ollama { model_name, .. } => model_name,
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
            #[cfg(feature = "ollama")]
            CompletionService::Ollama { .. } => "ollama",
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
            #[cfg(feature = "ollama")]
            CompletionService::Ollama { client, model_name } => {
                let model = client.completion_model(model_name);
                model
                    .completion_request(prompt)
                    .messages(chat_history)
                    .send()
                    .await
                    .map(|r| extract_text_content(&r.choice))
                    .map_err(map_err)
            }
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
            #[cfg(feature = "ollama")]
            CompletionService::Ollama { model_name, .. } => f
                .debug_struct("CompletionProvider::Ollama")
                .field("model", model_name)
                .finish(),
        }
    }
}
