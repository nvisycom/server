//! Provider client construction and credential validation.
//!
//! Each provider's rig client is built from an [`LlmConfig`] variant, then
//! [`VerifyClient::verify`] proves the credentials work against the provider.

use rig_core::client::Nothing;
use rig_core::client::verify::{VerifyClient, VerifyError};
use rig_core::providers::{anthropic, ollama, openai};

use crate::error::Error;
use crate::{AnthropicCredentials, LlmConfig, OpenAiCredentials};

/// Validates an LLM config by building its provider client and verifying the
/// credentials against the provider.
///
/// Used by the connection `test` endpoint. Returns `Ok(())` when the provider
/// accepts the credentials, or an [`Error`] describing the build or
/// verification failure.
pub async fn validate(config: &LlmConfig) -> Result<(), Error> {
    match config {
        LlmConfig::OpenAi {
            credentials,
            base_url,
            ..
        } => {
            let OpenAiCredentials { api_key } = credentials;
            let mut builder = openai::Client::builder().api_key(api_key);
            if let Some(base) = base_url {
                builder = builder.base_url(base);
            }
            let client = builder.build().map_err(|e| Error::Build(e.to_string()))?;
            verify(&client).await
        }
        LlmConfig::Anthropic {
            credentials,
            base_url,
            ..
        } => {
            let AnthropicCredentials { api_key } = credentials;
            let mut builder = anthropic::Client::builder().api_key(api_key);
            if let Some(base) = base_url {
                builder = builder.base_url(base);
            }
            let client = builder.build().map_err(|e| Error::Build(e.to_string()))?;
            verify(&client).await
        }
        LlmConfig::Ollama { base_url, .. } => {
            // Ollama is typically unauthenticated; `Nothing` is the keyless
            // api-key marker.
            let client = ollama::Client::builder()
                .api_key(Nothing)
                .base_url(base_url)
                .build()
                .map_err(|e| Error::Build(e.to_string()))?;
            verify(&client).await
        }
    }
}

/// Runs a provider client's verification, mapping the outcome to [`Error`].
async fn verify<C: VerifyClient>(client: &C) -> Result<(), Error> {
    match client.verify().await {
        Ok(()) => Ok(()),
        Err(VerifyError::InvalidAuthentication) => {
            Err(Error::Verify("invalid authentication".to_owned()))
        }
        Err(err) => Err(Error::Verify(err.to_string())),
    }
}
