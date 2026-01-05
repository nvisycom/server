//! Service provider configuration.

use anyhow::Context;
use nvisy_inference::InferenceService;
use nvisy_ollama::OllamaClient;
use nvisy_webhook::WebhookService;

use super::Cli;

/// Creates inference service from CLI configuration.
///
/// # Errors
///
/// Returns an error if the Ollama client cannot be initialized.
pub fn create_inference_service(cli: &Cli) -> anyhow::Result<InferenceService> {
    let client = OllamaClient::new(cli.ollama.clone()).context("failed to create Ollama client")?;
    Ok(InferenceService::from_provider(client))
}

/// Creates webhook service for external HTTP callbacks.
pub fn create_webhook_service() -> anyhow::Result<WebhookService> {
    use nvisy_reqwest::{ReqwestClient, ReqwestClientConfig};
    let config = ReqwestClientConfig::default();
    let client = ReqwestClient::new(config)?;
    Ok(client.into_service())
}
