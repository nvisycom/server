//! Provider-agnostic assistant client backed by a rig agent.
//!
//! [`AssistantClient`] is a thin, cloneable wrapper around any provider's rig
//! [`Agent`] that runs a multi-turn [`chat`](AssistantClient::chat), instrumented
//! with [`tracing`]: one runtime handle callers use regardless of which provider
//! backs it.

use std::sync::Arc;

use rig::agent::Agent;
use rig::completion::Message;

use super::ChatTurn;
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
}

/// Converts a provider-agnostic history into rig messages.
fn to_messages(history: Vec<ChatTurn>) -> Vec<Message> {
    history.into_iter().map(Message::from).collect()
}
