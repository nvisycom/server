//! Optical Character Recognition (OCR) abstractions.
//!
//! This module provides traits and types for extracting structured text from images
//! and documents. It supports various OCR capabilities including image text extraction,
//! document processing, language detection, and structured output.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

pub mod context;
pub mod error;
pub mod request;
pub mod response;

pub use context::{Context, Document, ProcessingOptions};
pub use error::{Error, Result};
pub use request::Request;
pub use response::Response;

use crate::health::ServiceHealth;

/// Type alias for a boxed OCR service.
pub type BoxedOcr = Arc<dyn Ocr + Send + Sync>;

/// Core trait for OCR operations.
#[async_trait]
pub trait Ocr {
    /// Extract text from an image or document.
    async fn extract_text(&self, request: &Request) -> Result<Response>;

    /// Detect the language of text in an image or document.
    async fn detect_language(&self, request: &Request) -> Result<context::LanguageDetectionResult>;

    /// Perform a health check on the service.
    async fn health_check(&self) -> Result<ServiceHealth>;
}

/// OCR service wrapper with additional functionality.
pub struct OcrService<T> {
    inner: T,
    retry_attempts: u32,
    timeout: Duration,
    enable_logging: bool,
    service_name: String,
}

impl<T> OcrService<T> {
    /// Create a new service wrapper.
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            retry_attempts: 3,
            timeout: Duration::from_secs(30),
            enable_logging: false,
            service_name: "ocr-service".to_string(),
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
impl<T> Ocr for OcrService<T>
where
    T: Ocr + Send + Sync,
{
    async fn extract_text(&self, request: &Request) -> Result<Response> {
        let mut last_error = None;

        for attempt in 1..=self.retry_attempts {
            if self.enable_logging {
                tracing::debug!(
                    "[{}] Extracting text, attempt {}/{}",
                    self.service_name,
                    attempt,
                    self.retry_attempts
                );
            }

            let start = std::time::Instant::now();

            match tokio::time::timeout(self.timeout, self.inner.extract_text(request)).await {
                Ok(Ok(response)) => {
                    if self.enable_logging {
                        tracing::debug!(
                            "[{}] Text extraction successful in {:?}",
                            self.service_name,
                            start.elapsed()
                        );
                    }
                    return Ok(response);
                }
                Ok(Err(error)) => {
                    if self.enable_logging {
                        tracing::warn!(
                            "[{}] Text extraction failed on attempt {}: {}",
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
                            "[{}] Text extraction timed out on attempt {}",
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

    async fn detect_language(&self, request: &Request) -> Result<context::LanguageDetectionResult> {
        tokio::time::timeout(self.timeout, self.inner.detect_language(request))
            .await
            .map_err(|_| Error::timeout())?
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
