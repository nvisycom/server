//! Embedding provider implementation for Ollama.

use jiff::Timestamp;
use nvisy_core::emb::{EmbeddingProvider, Request, Response};
use nvisy_core::{ServiceHealth, SharedContext};
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;

use crate::{OllamaClient, TRACING_TARGET_CLIENT};

#[async_trait::async_trait]
impl EmbeddingProvider for OllamaClient {
    async fn generate_embedding(
        &self,
        _context: &SharedContext,
        request: &Request,
    ) -> nvisy_core::Result<Response> {
        let model = self.embedding_model();
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            model = %model,
            "Generating embedding"
        );

        // Extract text from content
        let text = request.as_text().ok_or_else(|| {
            nvisy_core::Error::invalid_input()
                .with_message("Only text content is supported for embeddings")
        })?;

        let embed_request = GenerateEmbeddingsRequest::new(model.to_string(), text.into());

        let response = self
            .ollama()
            .generate_embeddings(embed_request)
            .await
            .map_err(|e| {
                nvisy_core::Error::external_error()
                    .with_message(format!("Ollama embedding error: {}", e))
            })?;

        let ended_at = Timestamp::now();

        // Take the first embedding (Ollama returns one embedding per request)
        let embedding = response.embeddings.into_iter().next().ok_or_else(|| {
            nvisy_core::Error::external_error().with_message("No embedding returned")
        })?;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            dimensions = embedding.len(),
            "Embedding generated"
        );

        Ok(request.reply(embedding).with_timing(started_at, ended_at))
    }

    async fn health_check(&self) -> nvisy_core::Result<ServiceHealth> {
        self.health_check()
            .await
            .map(|_| ServiceHealth::healthy())
            .map_err(|e| nvisy_core::Error::external_error().with_message(e.to_string()))
    }
}
