//! OCR processing via Ollama VLM.

use jiff::Timestamp;
use nvisy_inference::{Context, OcrRequest, OcrResponse, UsageStats};
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::images::Image;

use crate::{OllamaClient, TRACING_TARGET_CLIENT};

/// Default OCR prompt for text extraction.
const OCR_PROMPT: &str = "Extract all text from this image. Return only the extracted text, preserving the original layout and formatting as much as possible. Do not add any explanations or commentary.";

/// Processes an OCR request using Ollama's VLM.
pub async fn process(
    client: &OllamaClient,
    context: &Context,
    request: &OcrRequest,
) -> nvisy_inference::Result<OcrResponse> {
    let model = client.vlm_model();
    let started_at = Timestamp::now();

    let image_data = request.as_bytes();

    // Skip processing if no image data
    if image_data.is_empty() {
        return Ok(request.reply(String::new()));
    }

    tracing::debug!(
        target: TRACING_TARGET_CLIENT,
        request_id = %request.request_id,
        workspace_id = %context.workspace_id,
        model = %model,
        image_size = image_data.len(),
        content_type = ?request.content_type(),
        "Processing OCR request via Ollama VLM"
    );

    // Encode image to base64
    let image_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, image_data);

    let message = ChatMessage::user(OCR_PROMPT.to_string())
        .with_images(vec![Image::from_base64(&image_base64)]);

    let chat_request = ChatMessageRequest::new(model.to_string(), vec![message]);

    let result = client.ollama().send_chat_messages(chat_request).await;

    let ended_at = Timestamp::now();
    let processing_time = ended_at.duration_since(started_at);

    match result {
        Ok(response) => {
            let text = response.message.content;

            // Estimate tokens from prompt + response length
            let tokens = ((OCR_PROMPT.len() + text.len()) / 4) as u32;

            let usage = UsageStats::success(tokens, 1, processing_time);

            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                request_id = %request.request_id,
                extracted_text_len = text.len(),
                tokens = tokens,
                processing_time_ms = processing_time.as_millis(),
                "OCR request processed successfully"
            );

            Ok(OcrResponse::builder()
                .with_request_id(request.request_id)
                .with_text(text)
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
                "OCR request failed"
            );

            Err(nvisy_inference::Error::external_error()
                .with_message(format!("Ollama OCR error: {}", e)))
        }
    }
}
