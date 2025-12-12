//! Vision Language Model (VLM) abstractions.
//!
//! This module provides traits and types for working with multimodal AI models that
//! can process both images and text. It supports various VLM capabilities including
//! visual question answering, image description, visual reasoning, and multimodal
//! conversations.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::Stream;

pub mod context;
pub mod error;
pub mod request;
pub mod response;
pub mod tools;

pub use context::{Context, ProcessingOptions};
pub use error::{Error, Result};
pub use request::Request;
pub use response::Response;

use crate::health::ServiceHealth;

/// Type alias for a boxed VLM service.
pub type BoxedVlm = Arc<dyn Vlm + Send + Sync>;

/// Type alias for boxed response stream.
pub type BoxedStream<T> = Box<dyn Stream<Item = std::result::Result<T, Error>> + Send + Unpin>;

/// Core trait for VLM operations.
#[async_trait]
pub trait Vlm {
    /// Process a vision-language request.
    async fn process(&self, request: &Request) -> Result<Response>;

    /// Process a request with streaming response.
    async fn process_stream(&self, request: &Request) -> Result<BoxedStream<Response>>;

    /// List available models.
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// Get service name.
    fn service_name(&self) -> &str;

    /// Perform a health check on the service.
    async fn health_check(&self) -> Result<ServiceHealth>;
}

/// Information about an available model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    /// Model identifier.
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// Model description.
    pub description: Option<String>,
    /// Model capabilities.
    pub capabilities: ModelCapabilities,
    /// Model limits.
    pub limits: ModelLimits,
    /// Whether the model is currently available.
    pub available: bool,
    /// Model version.
    pub version: Option<String>,
}

impl Default for ModelInfo {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: None,
            capabilities: ModelCapabilities::default(),
            limits: ModelLimits::default(),
            available: true,
            version: None,
        }
    }
}

/// Model capabilities.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelCapabilities {
    /// Supports image description.
    pub image_description: bool,
    /// Supports visual question answering.
    pub visual_qa: bool,
    /// Supports object detection.
    pub object_detection: bool,
    /// Supports text extraction from images.
    pub text_extraction: bool,
    /// Supports image comparison.
    pub image_comparison: bool,
    /// Supports streaming responses.
    pub streaming: bool,
    /// Supported image formats.
    pub image_formats: Vec<String>,
    /// Maximum number of images per request.
    pub max_images: Option<u32>,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            image_description: true,
            visual_qa: true,
            object_detection: false,
            text_extraction: false,
            image_comparison: false,
            streaming: false,
            image_formats: vec!["image/jpeg".to_string(), "image/png".to_string()],
            max_images: Some(1),
        }
    }
}

/// Model limits and constraints.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelLimits {
    /// Maximum image size in bytes.
    pub max_image_size_bytes: Option<u64>,
    /// Maximum image dimensions (width x height).
    pub max_image_dimensions: Option<(u32, u32)>,
    /// Maximum context tokens.
    pub max_context_tokens: Option<u32>,
    /// Maximum output tokens.
    pub max_output_tokens: Option<u32>,
}

impl Default for ModelLimits {
    fn default() -> Self {
        Self {
            max_image_size_bytes: Some(10 * 1024 * 1024), // 10MB
            max_image_dimensions: Some((2048, 2048)),
            max_context_tokens: Some(4096),
            max_output_tokens: Some(1024),
        }
    }
}

/// VLM service wrapper with additional functionality.
pub struct VlmService<T> {
    inner: T,
    retry_attempts: u32,
    timeout: Duration,
    enable_logging: bool,
    service_name: String,
}

impl<T> VlmService<T> {
    /// Create a new service wrapper.
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            retry_attempts: 3,
            timeout: Duration::from_secs(30),
            enable_logging: false,
            service_name: "vlm-service".to_string(),
        }
    }

    /// Set retry policy.
    pub fn with_retry_policy(mut self, attempts: u32) -> Self {
        self.retry_attempts = attempts;
        self
    }

    /// Set timeout duration.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Enable or disable logging.
    pub fn with_logging(mut self, enable: bool) -> Self {
        self.enable_logging = enable;
        self
    }

    /// Set service name for logging and identification.
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Get a reference to the inner service.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Consume the wrapper and return the inner service.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[async_trait]
impl<T> Vlm for VlmService<T>
where
    T: Vlm + Send + Sync,
{
    async fn process(&self, request: &Request) -> Result<Response> {
        let mut last_error = None;

        for attempt in 1..=self.retry_attempts {
            if self.enable_logging {
                tracing::debug!(
                    "[{}] Processing VLM request, attempt {}/{}",
                    self.service_name,
                    attempt,
                    self.retry_attempts
                );
            }

            let start = std::time::Instant::now();

            match tokio::time::timeout(self.timeout, self.inner.process(request)).await {
                Ok(Ok(response)) => {
                    if self.enable_logging {
                        tracing::debug!(
                            "[{}] VLM processing successful in {:?}",
                            self.service_name,
                            start.elapsed()
                        );
                    }
                    return Ok(response);
                }
                Ok(Err(error)) => {
                    if self.enable_logging {
                        tracing::warn!(
                            "[{}] VLM processing failed on attempt {}: {}",
                            self.service_name,
                            attempt,
                            error
                        );
                    }

                    if !error.is_retryable() || attempt == self.retry_attempts {
                        return Err(error);
                    }

                    if let Some(delay) = error.retry_delay() {
                        tokio::time::sleep(delay).await;
                    }

                    last_error = Some(error);
                }
                Err(_) => {
                    let error = Error::timeout();
                    if self.enable_logging {
                        tracing::warn!(
                            "[{}] VLM processing timed out on attempt {}",
                            self.service_name,
                            attempt
                        );
                    }

                    if attempt == self.retry_attempts {
                        return Err(error);
                    }

                    tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                    last_error = Some(error);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| Error::internal_error()))
    }

    async fn process_stream(&self, request: &Request) -> Result<BoxedStream<Response>> {
        tokio::time::timeout(self.timeout, self.inner.process_stream(request))
            .await
            .map_err(|_| Error::timeout())?
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        self.inner.list_models().await
    }

    fn service_name(&self) -> &str {
        &self.service_name
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
