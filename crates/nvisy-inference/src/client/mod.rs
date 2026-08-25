//! Provider-agnostic inference client backed by a rig agent.
//!
//! [`InferenceClient`] is a thin, cloneable wrapper around any provider's rig
//! [`Agent`] that provides convenience methods for the most common operations —
//! one-shot [`prompt`](InferenceClient::prompt), multi-turn
//! [`chat`](InferenceClient::chat), and streaming
//! [`stream_chat`](InferenceClient::stream_chat). Every public method is
//! instrumented with [`tracing`] for observability. It plays the same role for
//! inference that `ObjectStoreClient` plays for object storage: one runtime
//! handle callers use regardless of which provider backs it.

mod token_stream;
mod turn;

use std::sync::Arc;

use async_stream::stream;
use futures::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::client::verify::{VerifyClient, VerifyError};
use rig::completion::message::Text;
use rig::completion::{Chat, Message, Prompt};
use rig::streaming::{StreamedAssistantContent, StreamingChat};

pub use self::token_stream::TokenStream;
pub use self::turn::{ChatTurn, Role};
use crate::error::Error;

/// Cloneable handle to any inference backend (OpenAI, Anthropic, Ollama, ...).
///
/// Wraps a rig [`Agent`] — which erases its provider's completion-model type
/// internally — behind a provider-agnostic interface, so callers issue prompts
/// without knowing which provider is configured.
#[derive(Clone)]
pub struct InferenceClient(Arc<Agent>);

impl InferenceClient {
    /// Wrap a rig [`Agent`].
    pub(crate) fn new(agent: Agent) -> Self {
        Self(Arc::new(agent))
    }

    /// Send a single prompt with no conversation history and return the model's
    /// text response.
    #[tracing::instrument(name = "inference.prompt", skip_all)]
    pub async fn prompt(&self, prompt: &str) -> Result<String, Error> {
        Prompt::prompt(&*self.0, prompt.to_owned())
            .await
            .map_err(|err| Error::Prompt(err.to_string()))
    }

    /// Run one chat turn against `history`, returning the model's text response.
    ///
    /// `history` is the prior conversation as [`ChatTurn`]s.
    #[tracing::instrument(name = "inference.chat", skip_all, fields(history_len = history.len()))]
    pub async fn chat(&self, prompt: &str, history: Vec<ChatTurn>) -> Result<String, Error> {
        let mut history = to_messages(history);
        Chat::chat(&*self.0, prompt.to_owned(), &mut history)
            .await
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
    #[tracing::instrument(name = "inference.stream_chat", skip_all, fields(history_len = history.len()))]
    pub fn stream_chat(&self, prompt: &str, history: Vec<ChatTurn>) -> TokenStream {
        // Own the agent handle in the generator so the result is `'static` and
        // can outlive this `InferenceClient` (moved into a response body). The
        // provider stream borrows the owned `Arc`, which the coroutine keeps
        // alive for the whole stream.
        let agent = Arc::clone(&self.0);
        let prompt = prompt.to_owned();
        let history = to_messages(history);
        let inner = stream! {
            let stream = StreamingChat::stream_chat(&*agent, prompt, history).await;
            for await item in stream {
                // Map rig's multi-turn stream down to bare text deltas: non-text
                // items (tool calls, reasoning, the final-response marker) carry
                // no user-visible text and are dropped.
                match item {
                    Ok(MultiTurnStreamItem::StreamAssistantItem(
                        StreamedAssistantContent::Text(Text { text, .. }),
                    )) => yield Ok(text),
                    Ok(_) => {}
                    Err(err) => yield Err(Error::Prompt(err.to_string())),
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

/// Verifies a built provider client's credentials against the provider.
///
/// Succeeds when the provider accepts the credentials; maps an authentication
/// rejection or any other verification failure to [`Error::Verify`].
pub async fn verify<C: VerifyClient>(client: &C) -> Result<(), Error> {
    match client.verify().await {
        Ok(()) => Ok(()),
        Err(VerifyError::InvalidAuthentication) => {
            Err(Error::Verify("invalid authentication".to_owned()))
        }
        Err(err) => Err(Error::Verify(err.to_string())),
    }
}
