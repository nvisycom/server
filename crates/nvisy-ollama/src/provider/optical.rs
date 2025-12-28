//! OCR (Optical Character Recognition) provider implementation using Ollama VLM.
//!
//! This module implements OCR functionality by using Ollama's vision-language models
//! to extract text from images.

use jiff::Timestamp;
use nvisy_core::ocr::{OcrProvider, Request, Response};
use nvisy_core::{ServiceHealth, SharedContext, UsageStats};
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::images::Image;

use crate::{OllamaClient, TRACING_TARGET_CLIENT};

/// Default OCR prompt for text extraction.
const OCR_PROMPT: &str = "Extract all text from this image. Return only the extracted text, preserving the original layout and formatting as much as possible. Do not add any explanations or commentary.";

#[async_trait::async_trait]
impl OcrProvider for OllamaClient {
    async fn process_ocr(
        &self,
        context: &SharedContext,
        request: &Request,
    ) -> nvisy_core::Result<Response> {
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

                Err(nvisy_core::Error::external_error()
                    .with_message(format!("Ollama OCR error: {}", e)))
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
