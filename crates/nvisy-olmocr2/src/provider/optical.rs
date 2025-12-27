//! OLMo OCR provider implementation.
//!
//! This module implements the [`OcrProvider`] trait for extracting text
//! from images and documents using OLMo v2 OCR models.

use nvisy_core::ocr::{BoxedStream, OcrProvider, Request, Response};
use nvisy_core::{ServiceHealth, SharedContext};

use crate::{OlmClient, TRACING_TARGET_CLIENT};

/// OLMo OCR provider.
///
/// Implements the [`OcrProvider`] trait for extracting text from images
/// and documents using OLMo v2 OCR models.
///
/// # Example
///
/// ```rust,ignore
/// use nvisy_olmocr2::{OlmClient, OlmConfig, OlemCredentials, OlmOcrProvider};
///
/// let config = OlmConfig::builder()
///     .with_base_url("https://api.olmo.ai/v2")?
///     .build()?;
/// let credentials = OlemCredentials::api_key("your-api-key");
/// let client = OlmClient::new(config, credentials).await?;
/// let provider = OlmOcrProvider::new(client);
/// ```
#[derive(Clone, Debug)]
pub struct OlmOcrProvider {
    client: OlmClient,
}

impl OlmOcrProvider {
    /// Creates a new OLMo OCR provider.
    ///
    /// # Arguments
    ///
    /// * `client` - The OLMo client to use for API calls
    pub fn new(client: OlmClient) -> Self {
        Self { client }
    }

    /// Returns a reference to the underlying client.
    pub fn client(&self) -> &OlmClient {
        &self.client
    }
}

/// OCR request payload containing image data.
pub trait OcrRequestPayload: Send + Sync {
    /// Get the image data as bytes.
    fn image_data(&self) -> &[u8];

    /// Get the MIME type of the image (e.g., "image/png", "image/jpeg").
    fn mime_type(&self) -> &str {
        "image/png"
    }
}

/// OCR response payload containing extracted text.
pub trait OcrResponsePayload: Send + Sync {
    /// Create from extracted text.
    fn from_text(text: String) -> Self;
}

#[async_trait::async_trait]
impl<Req, Resp> OcrProvider<Req, Resp> for OlmOcrProvider
where
    Req: OcrRequestPayload + 'static,
    Resp: OcrResponsePayload + 'static,
{
    async fn process_ocr(
        &self,
        _context: &SharedContext,
        request: Request<Req>,
    ) -> nvisy_core::Result<Response<Resp>> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            "Processing OCR request via OLMo"
        );

        // TODO: Implement actual OLMo OCR processing
        // This would involve:
        // 1. Sending the image to the OLMo OCR API
        // 2. Receiving the extracted text
        // 3. Converting to the response format

        let _image_data = request.payload.image_data();
        let _mime_type = request.payload.mime_type();

        // Placeholder: return empty text
        let text = String::new();

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            text_len = text.len(),
            "OCR request processed successfully"
        );

        Ok(Response::new(request.request_id, Resp::from_text(text)))
    }

    async fn process_ocr_stream(
        &self,
        _context: &SharedContext,
        request: Request<Req>,
    ) -> nvisy_core::Result<BoxedStream<Response<Resp>>> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            request_id = %request.request_id,
            "OCR streaming requested (not yet implemented)"
        );

        // TODO: Implement streaming OCR if OLMo API supports it
        Err(nvisy_core::Error::external_error()
            .with_message("OCR streaming not yet implemented for OLMo"))
    }

    async fn health_check(&self) -> nvisy_core::Result<ServiceHealth> {
        self.client
            .health_check()
            .await
            .map(|_| ServiceHealth::healthy())
            .map_err(|e| nvisy_core::Error::external_error().with_message(e.to_string()))
    }
}
