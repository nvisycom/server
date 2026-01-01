//! InferenceProvider implementation for Ollama.

use jiff::Timestamp;
use nvisy_service::inference::{
    EmbeddingRequest, EmbeddingResponse, InferenceProvider, OcrRequest, OcrResponse, VlmRequest,
    VlmResponse,
};
use nvisy_service::{ServiceHealth, SharedContext, UsageStats};
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest;
use ollama_rs::generation::images::Image;

use crate::{OllamaClient, TRACING_TARGET_CLIENT};

/// Default OCR prompt for text extraction.
const OCR_PROMPT: &str = "Extract all text from this image. Return only the extracted text, preserving the original layout and formatting as much as possible. Do not add any explanations or commentary.";

#[async_trait::async_trait]
impl InferenceProvider for OllamaClient {
    async fn generate_embedding(
        &self,
        context: &SharedContext,
        request: &EmbeddingRequest,
    ) -> nvisy_service::inference::Result<EmbeddingResponse> {
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
            nvisy_service::inference::Error::invalid_input()
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
                    nvisy_service::inference::Error::external_error()
                        .with_message("No embedding returned")
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

                Err(nvisy_service::inference::Error::external_error()
                    .with_message(format!("Ollama embedding error: {}", e)))
            }
        }
    }

    async fn process_ocr(
        &self,
        context: &SharedContext,
        request: &OcrRequest,
    ) -> nvisy_service::inference::Result<OcrResponse> {
        let model = self.vlm_model();
        let started_at = Timestamp::now();

        let image_data = request.as_bytes();

        // Skip processing if no image data
        if image_data.is_empty() {
            return Ok(request.reply(String::new()));
        }

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            model = %model,
            image_size = image_data.len(),
            content_type = ?request.content_type(),
            "Processing OCR request via Ollama VLM"
        );

        // Encode image to base64
        let image_base64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, image_data);

        // Use custom prompt if provided, otherwise use default OCR prompt
        let prompt = request.prompt.as_deref().unwrap_or(OCR_PROMPT).to_string();

        let message =
            ChatMessage::user(prompt.clone()).with_images(vec![Image::from_base64(&image_base64)]);

        let chat_request = ChatMessageRequest::new(model.to_string(), vec![message]);

        let result = self.ollama().send_chat_messages(chat_request).await;

        let ended_at = Timestamp::now();
        let processing_time = ended_at.duration_since(started_at);

        match result {
            Ok(response) => {
                let text = response.message.content;

                // Estimate tokens from prompt + response length
                let tokens = ((prompt.len() + text.len()) / 4) as u32;

                context
                    .record(UsageStats::success(tokens, 1, processing_time))
                    .await;

                tracing::debug!(
                    target: TRACING_TARGET_CLIENT,
                    request_id = %request.request_id,
                    extracted_text_len = text.len(),
                    tokens = tokens,
                    processing_time_ms = processing_time.as_millis(),
                    "OCR request processed successfully"
                );

                Ok(request.reply(text).with_timing(started_at, ended_at))
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
                    "OCR request failed"
                );

                Err(nvisy_service::inference::Error::external_error()
                    .with_message(format!("Ollama OCR error: {}", e)))
            }
        }
    }

    async fn process_vlm(
        &self,
        context: &SharedContext,
        request: &VlmRequest,
    ) -> nvisy_service::inference::Result<VlmResponse> {
        let model = self.vlm_model();
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            model = %model,
            document_count = request.document_count(),
            "Processing VLM request"
        );

        // Collect base64-encoded images from documents
        let images: Vec<Image> = request
            .documents
            .iter()
            .filter(|doc| doc.is_image())
            .map(|doc| {
                let base64 = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    doc.as_bytes(),
                );
                Image::from_base64(&base64)
            })
            .collect();

        let message = if images.is_empty() {
            ChatMessage::user(request.prompt.clone())
        } else {
            ChatMessage::user(request.prompt.clone()).with_images(images)
        };

        let chat_request = ChatMessageRequest::new(model.to_string(), vec![message]);

        let result = self.ollama().send_chat_messages(chat_request).await;

        let ended_at = Timestamp::now();
        let processing_time = ended_at.duration_since(started_at);

        match result {
            Ok(response) => {
                let text = response.message.content;

                // Estimate tokens from prompt + response length
                let tokens = ((request.prompt.len() + text.len()) / 4) as u32;
                // Count documents as runs
                let runs = request.document_count().max(1) as u32;

                context
                    .record(UsageStats::success(tokens, runs, processing_time))
                    .await;

                tracing::debug!(
                    target: TRACING_TARGET_CLIENT,
                    request_id = %request.request_id,
                    response_len = text.len(),
                    tokens = tokens,
                    runs = runs,
                    processing_time_ms = processing_time.as_millis(),
                    "VLM request processed"
                );

                Ok(request
                    .reply(text)
                    .with_timing(started_at, ended_at)
                    .with_finish_reason("stop"))
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
                    "VLM request failed"
                );

                Err(nvisy_service::inference::Error::external_error()
                    .with_message(format!("Ollama VLM error: {}", e)))
            }
        }
    }

    async fn health_check(&self) -> nvisy_service::inference::Result<ServiceHealth> {
        self.health_check()
            .await
            .map(|_| ServiceHealth::healthy())
            .map_err(|e| {
                nvisy_service::inference::Error::external_error().with_message(e.to_string())
            })
    }
}
