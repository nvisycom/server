//! rig-core trait implementations for CompletionProvider.

use rig::completion::{
    CompletionError, CompletionModel as RigCompletionModel, CompletionRequest, CompletionResponse,
};
use rig::message::Message;
use rig::one_or_many::OneOrMany;
#[cfg(feature = "ollama")]
use rig::prelude::CompletionClient;
use rig::streaming::StreamingCompletionResponse;

use super::provider::{CompletionProvider, CompletionService};
use super::response::{ProviderResponse, ProviderStreamingResponse};

impl RigCompletionModel for CompletionProvider {
    type Client = ();
    type Response = ProviderResponse;
    type StreamingResponse = ProviderStreamingResponse;

    fn make(_client: &Self::Client, _model: impl Into<String>) -> Self {
        // This is a no-op since CompletionProvider is constructed via its own methods
        panic!("CompletionProvider should be constructed via CompletionProvider::new()")
    }

    async fn completion(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        // Extract the prompt from the request's chat history (last message)
        let last_message = request.chat_history.last();
        let prompt = match last_message {
            Message::User { content } => content
                .iter()
                .filter_map(|c| match c {
                    rig::message::UserContent::Text(t) => Some(t.text()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
            _ => String::new(),
        };

        // Get chat history without the last message (which is the prompt)
        let chat_history: Vec<Message> = if request.chat_history.len() > 1 {
            request
                .chat_history
                .iter()
                .take(request.chat_history.len() - 1)
                .cloned()
                .collect()
        } else {
            vec![]
        };

        // Build the full prompt with preamble if present
        let full_prompt = match &request.preamble {
            Some(preamble) => format!("{}\n\n{}", preamble, prompt),
            None => prompt,
        };

        // Delegate to the underlying model based on variant
        let (choice, usage) = match self.inner() {
            CompletionService::OpenAi { model, .. } => {
                let resp = model
                    .completion(build_request(&full_prompt, &chat_history, &request))
                    .await?;
                (resp.choice, resp.usage)
            }
            CompletionService::Anthropic { model, .. } => {
                let resp = model
                    .completion(build_request(&full_prompt, &chat_history, &request))
                    .await?;
                (resp.choice, resp.usage)
            }
            CompletionService::Cohere { model, .. } => {
                let resp = model
                    .completion(build_request(&full_prompt, &chat_history, &request))
                    .await?;
                (resp.choice, resp.usage)
            }
            CompletionService::Gemini { model, .. } => {
                let resp = model
                    .completion(build_request(&full_prompt, &chat_history, &request))
                    .await?;
                (resp.choice, resp.usage)
            }
            CompletionService::Perplexity { model, .. } => {
                let resp = model
                    .completion(build_request(&full_prompt, &chat_history, &request))
                    .await?;
                (resp.choice, resp.usage)
            }
            #[cfg(feature = "ollama")]
            CompletionService::Ollama { client, model_name } => {
                let model = client.completion_model(model_name);
                let resp = model
                    .completion(build_request(&full_prompt, &chat_history, &request))
                    .await?;
                (resp.choice, resp.usage)
            }
        };

        Ok(CompletionResponse {
            choice,
            usage,
            raw_response: ProviderResponse {
                provider: self.provider_name().to_string(),
                model: self.model_name().to_string(),
            },
        })
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        // For now, streaming is not fully implemented - we'd need to unify the streaming types
        // This is a placeholder that returns an error
        let _ = request;
        Err(CompletionError::RequestError(
            "Streaming not yet implemented for CompletionProvider".into(),
        ))
    }
}

/// Builds a completion request for delegation to underlying models.
fn build_request(
    prompt: &str,
    chat_history: &[Message],
    original: &CompletionRequest,
) -> CompletionRequest {
    CompletionRequest {
        preamble: None, // Already incorporated into prompt
        chat_history: {
            let mut history = chat_history.to_vec();
            history.push(Message::User {
                content: OneOrMany::one(rig::message::UserContent::text(prompt)),
            });
            OneOrMany::many(history).unwrap_or_else(|_| {
                OneOrMany::one(Message::User {
                    content: OneOrMany::one(rig::message::UserContent::text(prompt)),
                })
            })
        },
        documents: original.documents.clone(),
        tools: original.tools.clone(),
        temperature: original.temperature,
        max_tokens: original.max_tokens,
        tool_choice: original.tool_choice.clone(),
        additional_params: original.additional_params.clone(),
    }
}
