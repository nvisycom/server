//! VLM (Vision Language Model) provider implementation and payload traits.

use nvisy_core::vlm::{BoxedStream, Request, Response, VlmProvider};
use nvisy_core::{ServiceHealth, SharedContext};
use ollama_rs::generation::chat::ChatMessage;
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::images::Image;

use crate::{OllamaClient, TRACING_TARGET_CLIENT};

/// Trait for types that can be used as VLM request payloads.
pub trait VlmRequestPayload: Send + Sync {
    /// Get the text prompt.
    fn prompt(&self) -> &str;

    /// Get base64-encoded images (if any).
    fn images(&self) -> Vec<String> {
        vec![]
    }
}

/// Trait for types that can be constructed from VLM results.
pub trait VlmResponsePayload: Send + Sync {
    /// Create from generated text.
    fn from_text(text: String) -> Self;
}

impl VlmResponsePayload for String {
    fn from_text(text: String) -> Self {
        text
    }
}

#[async_trait::async_trait]
impl<Req, Resp> VlmProvider<Req, Resp> for OllamaClient
where
    Req: VlmRequestPayload + 'static,
    Resp: VlmResponsePayload + 'static,
{
    async fn process_vlm(
        &self,
        _context: &SharedContext,
        request: &Request<Req>,
    ) -> nvisy_core::Result<Response<Resp>> {
        let model = self.vlm_model();

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            model = %model,
            "Processing VLM request"
        );

        let prompt = request.payload.prompt();
        let images = request.payload.images();

        let message = if images.is_empty() {
            ChatMessage::user(prompt.to_string())
        } else {
            let ollama_images: Vec<Image> = images.into_iter().map(Image::from_base64).collect();
            ChatMessage::user(prompt.to_string()).with_images(ollama_images)
        };

        let chat_request = ChatMessageRequest::new(model.to_string(), vec![message]);

        let response = self
            .ollama()
            .send_chat_messages(chat_request)
            .await
            .map_err(|e| {
                nvisy_core::Error::external_error().with_message(format!("Ollama VLM error: {}", e))
            })?;

        let text = response.message.content;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            response_len = text.len(),
            "VLM request processed"
        );

        Ok(Response::new(request.request_id, Resp::from_text(text)))
    }

    async fn process_vlm_stream(
        &self,
        _context: &SharedContext,
        request: &Request<Req>,
    ) -> nvisy_core::Result<BoxedStream<Response<Resp>>> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            "VLM streaming not yet implemented"
        );

        Err(nvisy_core::Error::external_error().with_message("VLM streaming not yet implemented"))
    }

    async fn health_check(&self) -> nvisy_core::Result<ServiceHealth> {
        self.health_check()
            .await
            .map(|_| ServiceHealth::healthy())
            .map_err(|e| nvisy_core::Error::external_error().with_message(e.to_string()))
    }
}
