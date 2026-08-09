//! Provider-agnostic inference client backed by a rig agent.
//!
//! [`InferenceClient`] is a thin, cloneable wrapper around any provider's rig
//! [`Agent`] that provides convenience methods for the most common operations —
//! one-shot [`prompt`](InferenceClient::prompt) and multi-turn
//! [`chat`](InferenceClient::chat). Every public method is instrumented with
//! [`tracing`] for observability. It plays the same role for inference that
//! `ObjectStoreClient` plays for object storage: one runtime handle callers use
//! regardless of which provider backs it.

use std::sync::Arc;

use futures::future::BoxFuture;
use rig::agent::Agent;
use rig::client::verify::{VerifyClient, VerifyError};
/// A single conversation message, re-exported so callers can build the
/// [`chat`](InferenceClient::chat) history without depending on `rig` directly.
pub use rig::completion::Message;
use rig::completion::{Chat, CompletionModel, Prompt, PromptError};

use crate::error::Error;

/// Object-safe view of a rig [`Agent`], erasing the provider's concrete
/// completion-model type so a single handle can hold any backend.
trait ErasedAgent: Send + Sync {
    /// Send a single prompt with no prior context.
    fn prompt(&self, prompt: String) -> BoxFuture<'_, Result<String, PromptError>>;

    /// Run one chat turn against `history`, appending the committed messages.
    fn chat<'a>(
        &'a self,
        prompt: String,
        history: &'a mut Vec<Message>,
    ) -> BoxFuture<'a, Result<String, PromptError>>;
}

impl<M> ErasedAgent for Agent<M>
where
    M: CompletionModel + 'static,
{
    fn prompt(&self, prompt: String) -> BoxFuture<'_, Result<String, PromptError>> {
        Box::pin(async move { Prompt::prompt(self, prompt).await })
    }

    fn chat<'a>(
        &'a self,
        prompt: String,
        history: &'a mut Vec<Message>,
    ) -> BoxFuture<'a, Result<String, PromptError>> {
        Box::pin(async move { Chat::chat(self, prompt, history).await })
    }
}

/// Cloneable handle to any inference backend (OpenAI, Anthropic, Ollama, ...).
///
/// Wraps a provider's rig agent behind a provider-agnostic interface, so callers
/// issue prompts without knowing which provider is configured.
#[derive(Clone)]
pub struct InferenceClient(Arc<dyn ErasedAgent>);

impl InferenceClient {
    /// Wrap a concrete rig [`Agent`].
    pub(crate) fn new<M>(agent: Agent<M>) -> Self
    where
        M: CompletionModel + 'static,
    {
        Self(Arc::new(agent))
    }

    /// Send a single prompt with no conversation history and return the model's
    /// text response.
    #[tracing::instrument(name = "inference.prompt", skip_all)]
    pub async fn prompt(&self, prompt: &str) -> Result<String, Error> {
        self.0
            .prompt(prompt.to_owned())
            .await
            .map_err(|err| Error::Prompt(err.to_string()))
    }

    /// Run one chat turn against `history`, returning the model's text response.
    ///
    /// `history` is caller-owned and updated in place: the prompt and the
    /// messages the model commits this turn are appended to it, so passing the
    /// same `Vec` across calls continues the conversation.
    #[tracing::instrument(name = "inference.chat", skip_all, fields(history_len = history.len()))]
    pub async fn chat(&self, prompt: &str, history: &mut Vec<Message>) -> Result<String, Error> {
        self.0
            .chat(prompt.to_owned(), history)
            .await
            .map_err(|err| Error::Prompt(err.to_string()))
    }
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
