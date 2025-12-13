//! OCR service wrapper with retry logic, timeouts, and observability.
//!
//! This module provides a generic wrapper around OCR implementations that adds
//! production-ready features like automatic retries, configurable timeouts,
//! and optional logging.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::ocr::{Ocr, OcrService};
//! use std::time::Duration;
//!
//! let ocr = MyOcrImpl::new();
//! let service = OcrService::new(ocr)
//!     .with_retry_policy(3)
//!     .with_timeout(Duration::from_secs(30))
//!     .with_logging(true)
//!     .with_service_name("my-ocr-service");
//!
//! // Use the wrapped service
//! let response = service.process_with_ocr(request).await?;
//! ```

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use super::{BoxedStream, Error, Ocr, Request, Response, Result};
use crate::health::ServiceHealth;

/// OCR service wrapper with additional functionality.
///
/// This wrapper adds retry logic, timeout handling, and optional logging
/// to any OCR implementation. It's generic over the request and response
/// types to maintain compatibility with the underlying OCR trait.
///
/// The inner service is wrapped in an `Arc`, making this wrapper cheap to clone.
///
/// # Type Parameters
///
/// * `T` - The inner OCR service implementation
/// * `Req` - The request payload type
/// * `Resp` - The response payload type
#[derive(Clone)]
pub struct OcrService<T, Req, Resp> {
    inner: Arc<T>,
    retry_attempts: u32,
    timeout: Duration,
    enable_logging: bool,
    service_name: String,
    _phantom: PhantomData<(Req, Resp)>,
}

impl<T, Req, Resp> OcrService<T, Req, Resp> {
    /// Create a new service wrapper with default configuration.
    ///
    /// Default configuration:
    /// - 3 retry attempts
    /// - 30 second timeout
    /// - Logging disabled
    /// - Service name: "ocr-service"
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(inner),
            retry_attempts: 3,
            timeout: Duration::from_secs(30),
            enable_logging: false,
            service_name: "ocr-service".to_string(),
            _phantom: PhantomData,
        }
    }

    /// Set the number of retry attempts for failed requests.
    ///
    /// Only retryable errors (network issues, timeouts, rate limits) will be retried.
    /// Non-retryable errors (authentication, invalid input) fail immediately.
    pub fn with_retry_policy(mut self, attempts: u32) -> Self {
        self.retry_attempts = attempts;
        self
    }

    /// Set the timeout duration for OCR operations.
    ///
    /// Operations that exceed this duration will return a timeout error.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Enable or disable logging for OCR operations.
    ///
    /// When enabled, logs debug and warning messages for request attempts,
    /// successes, failures, and timeouts.
    pub fn with_logging(mut self, enable: bool) -> Self {
        self.enable_logging = enable;
        self
    }

    /// Set the service name used in logs and for identification.
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Get a reference to the inner OCR service.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Consume the wrapper and return the inner OCR service.
    ///
    /// If there are other references to the inner service, this will clone it.
    pub fn into_inner(self) -> T
    where
        T: Clone,
    {
        Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone())
    }
}

impl<T, Req, Resp> Ocr<Req, Resp> for OcrService<T, Req, Resp>
where
    T: Ocr<Req, Resp> + Send + Sync,
    Req: Send + Sync + Clone,
    Resp: Send + Sync,
{
    async fn process_with_ocr(&self, request: Request<Req>) -> Result<Response<Resp>> {
        let mut last_error = None;

        for attempt in 1..=self.retry_attempts {
            if self.enable_logging {
                tracing::debug!(
                    "[{}] Processing OCR request, attempt {}/{}",
                    self.service_name,
                    attempt,
                    self.retry_attempts
                );
            }

            let start = std::time::Instant::now();

            // Clone request for each attempt
            let request_clone = request.clone();

            match tokio::time::timeout(self.timeout, self.inner.process_with_ocr(request_clone))
                .await
            {
                Ok(Ok(response)) => {
                    if self.enable_logging {
                        tracing::debug!(
                            "[{}] OCR processing successful in {:?}",
                            self.service_name,
                            start.elapsed()
                        );
                    }
                    return Ok(response);
                }
                Ok(Err(error)) => {
                    if self.enable_logging {
                        tracing::warn!(
                            "[{}] OCR processing failed on attempt {}: {}",
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
                            "[{}] OCR processing timed out on attempt {}",
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

        Err(last_error.unwrap_or_else(Error::internal_error))
    }

    async fn process_stream(&self, request: Request<Req>) -> Result<BoxedStream<Response<Resp>>> {
        tokio::time::timeout(self.timeout, self.inner.process_stream(request))
            .await
            .map_err(|_| Error::timeout())?
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
