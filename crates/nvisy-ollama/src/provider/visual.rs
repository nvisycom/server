//! VLM (Vision Language Model) provider implementation.

use jiff::Timestamp;
use nvisy_core::vlm::{Request, Response, VlmProvider};
use nvisy_core::{ServiceHealth, SharedContext, UsageStats};
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::images::Image;

use crate::{OllamaClient, TRACING_TARGET_CLIENT};

#[async_trait::async_trait]
impl VlmProvider for OllamaClient {
    async fn process_vlm(
        &self,
        context: &SharedContext,
        request: &Request,
    ) -> nvisy_core::Result<Response> {
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

                Err(nvisy_core::Error::external_error()
                    .with_message(format!("Ollama VLM error: {}", e)))
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
