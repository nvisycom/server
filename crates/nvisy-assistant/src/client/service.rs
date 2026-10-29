//! Provider-agnostic assistant client backed by a rig agent.
//!
//! [`AssistantClient`] is a thin, cloneable wrapper around any provider's rig
//! [`Agent`] that provides convenience methods for the most common operations —
//! one-shot [`prompt`](AssistantClient::prompt), multi-turn
//! [`chat`](AssistantClient::chat), and streaming
//! [`stream_chat`](AssistantClient::stream_chat). Every public method is
//! instrumented with [`tracing`] for observability. It plays the same role for
//! language-model access that `ObjectStoreClient` plays for object storage: one
//! runtime handle callers use regardless of which provider backs it.

use std::sync::Arc;

use async_stream::stream;
use futures::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::completion::Message;
use rig::streaming::{Item, StreamEvent};

use super::{ChatTurn, TokenStream};
use crate::error::{Error, Result};

/// Cloneable handle to any language-model backend (`OpenAI`, Anthropic, Ollama, ...).
///
/// Wraps a rig [`Agent`] — which erases its provider's completion-model type
/// internally — behind a provider-agnostic interface, so callers issue prompts
/// without knowing which provider is configured.
#[derive(Clone)]
pub struct AssistantClient(Arc<Agent>);

impl AssistantClient {
    /// Wrap a rig [`Agent`].
    pub(crate) fn new(agent: Agent) -> Self {
        Self(Arc::new(agent))
    }

    /// Send a single prompt with no conversation history and return the model's
    /// text response.
    ///
    /// # Errors
    ///
    /// A `Prompt` error if the completion request fails at runtime.
    #[tracing::instrument(name = "assistant.prompt", skip_all)]
    pub async fn prompt(&self, prompt: &str) -> Result<String> {
        self.0
            .prompt(prompt.to_owned())
            .await
            .map(|response| response.output())
            .map_err(|err| Error::Prompt(err.to_string()))
    }

    /// Run one chat turn against `history`, returning the model's text response.
    ///
    /// `history` is the prior conversation as [`ChatTurn`]s.
    ///
    /// # Errors
    ///
    /// A `Prompt` error if the completion request fails at runtime.
    #[tracing::instrument(name = "assistant.chat", skip_all, fields(history_len = history.len()))]
    pub async fn chat(&self, prompt: &str, history: Vec<ChatTurn>) -> Result<String> {
        let mut history = to_messages(history);
        self.0
            .chat(prompt.to_owned(), &mut history)
            .await
            .map(|response| response.output())
            .map_err(|err| Error::Prompt(err.to_string()))
    }

    /// Stream one chat turn against `history`, yielding the model's response as
    /// text deltas.
    ///
    /// Returns immediately with a [`TokenStream`]; the request opens lazily when
    /// the stream is first polled. Each item is a token chunk as it arrives, and
    /// a failure mid-generation ends the stream with an `Err`. `history` is the
    /// prior conversation as [`ChatTurn`]s; persist the user prompt and the
    /// assembled reply on the caller side.
    #[tracing::instrument(name = "assistant.stream_chat", skip_all, fields(history_len = history.len()))]
    pub fn stream_chat(&self, prompt: &str, history: Vec<ChatTurn>) -> TokenStream {
        // Own the agent handle in the generator so the result is `'static` and
        // can outlive this `AssistantClient` (moved into a response body). The
        // provider stream borrows the owned `Arc`, which the coroutine keeps
        // alive for the whole stream.
        let agent = Arc::clone(&self.0);
        let prompt = prompt.to_owned();
        let history = to_messages(history);
        let inner = stream! {
            let stream = agent.prompt(prompt).history(history).stream();
            for await item in stream {
                // Map rig's multi-turn stream down to bare text deltas: non-text
                // items (tool calls, reasoning, the final-response marker) carry
                // no user-visible text and are dropped.
                match item {
                    Ok(MultiTurnStreamItem::StreamAssistantItem(Item::Event(
                        StreamEvent::Text { text, .. },
                    ))) => yield Ok(text),
                    Ok(_) => {}
                    // An error is terminal: yield it and stop, per TokenStream's
                    // contract, rather than polling the rig stream again.
                    Err(err) => {
                        yield Err(Error::Prompt(err.to_string()));
                        break;
                    }
                }
            }
        };
        TokenStream::new(inner.boxed())
    }
}

/// Converts a provider-agnostic history into rig messages.
fn to_messages(history: Vec<ChatTurn>) -> Vec<Message> {
    history.into_iter().map(Message::from).collect()
}
