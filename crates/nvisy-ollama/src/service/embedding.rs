//! Embedding generation via Ollama.

use jiff::Timestamp;
use nvisy_inference::{Context, EmbeddingRequest, EmbeddingResponse, UsageStats};
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;

use crate::{OllamaClient, TRACING_TARGET_CLIENT};

/// Generates an embedding for the given request.
pub async fn generate(
    client: &OllamaClient,
    context: &Context,
    request: &EmbeddingRequest,
) -> nvisy_inference::Result<EmbeddingResponse> {
    let model = client.embedding_model();
    let started_at = Timestamp::now();

    tracing::debug!(
        target: TRACING_TARGET_CLIENT,
        request_id = %request.request_id,
        workspace_id = %context.workspace_id,
        model = %model,
        "Generating embedding"
    );

    // Extract text from content
    let text = request.as_text().ok_or_else(|| {
        nvisy_inference::Error::invalid_input()
            .with_message("Only text content is supported for embeddings")
    })?;

    let embed_request = GenerateEmbeddingsRequest::new(model.to_string(), text.into());
    let result = client.ollama().generate_embeddings(embed_request).await;

    let ended_at = Timestamp::now();
    let processing_time = ended_at.duration_since(started_at);

    match result {
        Ok(response) => {
            // Take the first embedding (Ollama returns one embedding per request)
            let embedding = response.embeddings.into_iter().next().ok_or_else(|| {
                nvisy_inference::Error::external_error().with_message("No embedding returned")
            })?;

            // Estimate tokens from text length (rough approximation: ~4 chars per token)
            let tokens = (text.len() / 4) as u32;

            let usage = UsageStats::success(tokens, 1, processing_time);

            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                request_id = %request.request_id,
                dimensions = embedding.len(),
                tokens = tokens,
                processing_time_ms = processing_time.as_millis(),
                "Embedding generated"
            );

            Ok(EmbeddingResponse::builder()
                .with_request_id(request.request_id)
                .with_embedding(embedding)
                .with_usage(usage)
                .build()
                .expect("valid response"))
        }
        Err(e) => {
            tracing::error!(
                target: TRACING_TARGET_CLIENT,
                request_id = %request.request_id,
                error = %e,
                processing_time_ms = processing_time.as_millis(),
                "Embedding generation failed"
            );

            Err(nvisy_inference::Error::external_error()
                .with_message(format!("Ollama embedding error: {}", e)))
        }
    }
}
