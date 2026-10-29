//! LLM provider clients and the dispatch trait over them.

mod anthropic;
mod ollama;
mod openai;

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
use rig::DynModel;
use rig::agent::AgentBuilder;
use rig::error::ProviderError;
use rig::operation::Completion;

use crate::client::AssistantClient;
use crate::error::{Error, Result};

/// An LLM provider that builds a verifiable, prompt-capable client from
/// typed credentials.
///
/// Each provider is a newtype wrapping its rig client. [`verify`](Client::verify)
/// checks its credentials, and [`model`](Client::model) turns it into a
/// provider-agnostic `AssistantClient`.
pub(crate) trait Client: Send + Sync + 'static {
    /// The credential this provider connects with. A single-secret provider uses
    /// `str` (the API key); a keyless one (Ollama) uses a unit type.
    type Credentials: Send + Sync + ?Sized;

    /// Unique identifier (e.g. `openai`, `anthropic`).
    const ID: &str;

    /// Build a client from credentials and an optional base-URL override.
    fn connect(credentials: &Self::Credentials, base_url: Option<&str>) -> Self
    where
        Self: Sized;

    /// Verify the client's credentials against the provider.
    ///
    /// Succeeds when the provider accepts the credentials; maps an authentication
    /// rejection or any other failure to [`Error::Verify`].
    fn verify(&self) -> impl Future<Output = Result<()>> + Send;

    /// The rig completion model this provider serves `model` with, erased for
    /// the agent builder.
    fn completion_model(&self, model: &str) -> DynModel<Completion>;

    /// Turn this provider client into a provider-agnostic [`AssistantClient`]
    /// bound to `model`.
    fn model(&self, model: &str) -> AssistantClient {
        AssistantClient::new(AgentBuilder::new(self.completion_model(model)).build())
    }
}

/// Maps a provider verification result to [`Error::Verify`], naming an
/// authentication rejection distinctly.
fn map_verify(result: std::result::Result<(), ProviderError>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(ProviderError::InvalidAuthentication(_)) => {
            Err(Error::Verify("invalid authentication".to_owned()))
        }
        Err(err) => Err(Error::Verify(err.to_string())),
    }
}

/// Keyless credentials for Ollama, which is typically unauthenticated.
#[derive(Debug, Clone, Copy)]
pub struct OllamaCredentials;
