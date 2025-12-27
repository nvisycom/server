//! Embedding provider implementation for Ollama.

use jiff::Timestamp;
use nvisy_core::emb::{EmbeddingProvider, Request, Response};
use nvisy_core::{ServiceHealth, SharedContext, UsageStats};
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;

use crate::{OllamaClient, TRACING_TARGET_CLIENT};

#[async_trait::async_trait]
impl EmbeddingProvider for OllamaClient {
    async fn generate_embedding(
        &self,
        context: &SharedContext,
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

        let result = self.ollama().generate_embeddings(embed_request).await;

        let ended_at = Timestamp::now();
        let processing_time = ended_at.duration_since(started_at);

        match result {
            Ok(response) => {
                // Take the first embedding (Ollama returns one embedding per request)
                let embedding = response.embeddings.into_iter().next().ok_or_else(|| {
                    nvisy_core::Error::external_error().with_message("No embedding returned")
                })?;

                // Estimate tokens from text length (rough approximation: ~4 chars per token)
                let tokens = (text.len() / 4) as u32;

                context
                    .record(UsageStats::success(tokens, 1, processing_time))
                    .await;

                tracing::debug!(
                    target: TRACING_TARGET_CLIENT,
                    request_id = %request.request_id,
                    dimensions = embedding.len(),
                    tokens = tokens,
                    processing_time_ms = processing_time.as_millis(),
                    "Embedding generated"
                );

                Ok(request.reply(embedding).with_timing(started_at, ended_at))
            }
            Err(e) => {
                context
                    .record(UsageStats::failure(0, processing_time))
                    .await;

                tracing::error!(
                    target: TRACING_TARGET_CLIENT,
                    request_id = %request.request_id,
                    error = %e,
                    processing_time_ms = processing_time.as_millis(),
                    "Embedding generation failed"
                );

                Err(nvisy_core::Error::external_error()
                    .with_message(format!("Ollama embedding error: {}", e)))
            }
        }
    }

    async fn health_check(&self) -> nvisy_core::Result<ServiceHealth> {
        self.health_check()
            .await
            .map(|_| ServiceHealth::healthy())
            .map_err(|e| nvisy_core::Error::external_error().with_message(e.to_string()))
    }
}
