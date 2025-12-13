//! VLM service wrapper with retry logic, timeouts, and observability.
//!
//! This module provides a wrapper around VLM implementations that adds
//! production-ready features like automatic retries, configurable timeouts,
//! and optional logging.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::vlm::{Vlm, VlmService};
//! use std::time::Duration;
//!
//! let vlm = MyVlmImpl::new();
//! let service = VlmService::new(vlm)
//!     .with_retry_policy(3)
//!     .with_timeout(Duration::from_secs(30))
//!     .with_logging(true)
//!     .with_service_name("my-vlm-service");
//!
//! // Use the wrapped service
//! let response = service.process(&request).await?;
//! ```

use std::sync::Arc;
use std::time::Duration;

use super::{BoxedStream, Error, Request, Response, Result, Vlm};
use crate::health::ServiceHealth;

/// Service wrapper with additional functionality.
///
/// This wrapper adds retry logic, timeout handling, and optional logging
/// to any VLM implementation.
///
/// The inner service is wrapped in an `Arc`, making this wrapper cheap to clone.
///
/// # Type Parameters
///
/// * `T` - The inner VLM service implementation
#[derive(Clone)]
pub struct Service<T> {
    inner: Arc<T>,
    retry_attempts: u32,
    timeout: Duration,
    enable_logging: bool,
    service_name: String,
}

impl<T> Service<T> {
    /// Create a new service wrapper with default configuration.
    ///
    /// Default configuration:
    /// - 3 retry attempts
    /// - 30 second timeout
    /// - Logging disabled
    /// - Service name: "vlm-service"
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(inner),
            retry_attempts: 3,
            timeout: Duration::from_secs(30),
            enable_logging: false,
            service_name: "vlm-service".to_string(),
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

    /// Set the timeout duration for VLM operations.
    ///
    /// Operations that exceed this duration will return a timeout error.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Enable or disable logging for VLM operations.
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

    /// Get a reference to the inner VLM service.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Consume the wrapper and return the inner VLM service.
    ///
    /// If there are other references to the inner service, this will clone it.
    pub fn into_inner(self) -> T
    where
        T: Clone,
    {
        Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone())
    }
}

impl<T> Vlm for Service<T>
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

        Err(last_error.unwrap_or_else(Error::internal_error))
    }

    async fn process_stream(&self, request: &Request) -> Result<BoxedStream<Response>> {
        tokio::time::timeout(self.timeout, self.inner.process_stream(request))
            .await
            .map_err(|_| Error::timeout())?
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
