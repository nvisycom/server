//! [`ErasedAgent`]: an object-safe view of a rig [`Agent`], erasing the
//! provider's concrete completion-model type so a single handle can hold any
//! backend.

use futures::future::BoxFuture;
use futures::stream::{BoxStream, StreamExt};
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::completion::message::Text;
use rig::completion::{Chat, CompletionModel, GetTokenUsage, Message, Prompt, PromptError};
use rig::streaming::{StreamedAssistantContent, StreamingChat};

use crate::error::Error;

/// Object-safe view of a rig [`Agent`], erasing the provider's concrete
/// completion-model type so a single handle can hold any backend.
pub(crate) trait ErasedAgent: Send + Sync {
    /// Send a single prompt with no prior context.
    fn prompt(&self, prompt: String) -> BoxFuture<'_, Result<String, PromptError>>;

    /// Run one chat turn against `history`, appending the committed messages.
    fn chat<'a>(
        &'a self,
        prompt: String,
        history: &'a mut Vec<Message>,
    ) -> BoxFuture<'a, Result<String, PromptError>>;

    /// Stream one chat turn against `history` as text deltas.
    fn stream_chat<'a>(
        &'a self,
        prompt: String,
        history: Vec<Message>,
    ) -> BoxFuture<'a, BoxStream<'a, Result<String, Error>>>;
}

impl<M> ErasedAgent for Agent<M>
where
    M: CompletionModel + 'static,
    M::StreamingResponse: GetTokenUsage,
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

    fn stream_chat<'a>(
        &'a self,
        prompt: String,
        history: Vec<Message>,
    ) -> BoxFuture<'a, BoxStream<'a, Result<String, Error>>> {
        Box::pin(async move {
            // Map rig's multi-turn stream down to bare text deltas here, inside
            // the concrete-`M` impl, so the boxed stream is provider-agnostic and
            // the trait stays object-safe.
            let stream = StreamingChat::stream_chat(self, prompt, history).await;
            let deltas = stream.filter_map(|item| async move {
                match item {
                    Ok(MultiTurnStreamItem::StreamAssistantItem(
                        StreamedAssistantContent::Text(Text { text, .. }),
                    )) => Some(Ok(text)),
                    // Non-text items (tool calls, reasoning, the final response
                    // marker) carry no user-visible text: drop them.
                    Ok(_) => None,
                    Err(err) => Some(Err(Error::Prompt(err.to_string()))),
                }
            });
            deltas.boxed()
        })
    }
}
