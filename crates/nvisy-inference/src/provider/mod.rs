//! Inference provider clients and the dispatch trait over them.

mod anthropic;
mod ollama;
mod openai;

use std::ops::Deref;

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
use rig::client::verify::{VerifyClient, VerifyError};
use rig::client::{AgentClientExt, CompletionClient};

use crate::client::InferenceClient;
use crate::error::{Error, Result};

/// An inference provider that builds a verifiable, prompt-capable client from
/// typed credentials.
///
/// Each provider is a newtype wrapping its rig client, which derefs to that
/// client. [`verify`](Client::verify) checks its credentials, and
/// [`model`](Client::model) turns it into a provider-agnostic `InferenceClient`.
pub(crate) trait Client:
    Deref<Target: Sized + VerifyClient + CompletionClient<CompletionModel: 'static>>
    + Send
    + Sync
    + 'static
{
    /// The credential this provider connects with. A single-secret provider uses
    /// `str` (the API key); a keyless one (Ollama) uses a unit type.
    type Credentials: Send + Sync + ?Sized;

    /// Unique identifier (e.g. `openai`, `anthropic`).
    const ID: &str;

    /// Build a client from credentials and an optional base-URL override.
    fn connect(credentials: &Self::Credentials, base_url: Option<&str>) -> Result<Self>
    where
        Self: Sized;

    /// Verify the client's credentials against the provider.
    ///
    /// Succeeds when the provider accepts the credentials; maps an authentication
    /// rejection or any other failure to [`Error::Verify`].
    fn verify(&self) -> impl Future<Output = Result<()>> + Send {
        async move {
            match (**self).verify().await {
                Ok(()) => Ok(()),
                Err(VerifyError::InvalidAuthentication) => {
                    Err(Error::Verify("invalid authentication".to_owned()))
                }
                Err(err) => Err(Error::Verify(err.to_string())),
            }
        }
    }

    /// Turn this provider client into a provider-agnostic [`InferenceClient`]
    /// bound to `model`.
    fn model(&self, model: &str) -> InferenceClient {
        InferenceClient::new(AgentClientExt::agent(&**self, model).build())
    }
}

/// Keyless credentials for Ollama, which is typically unauthenticated.
#[derive(Debug, Clone, Copy)]
pub struct OllamaCredentials;
