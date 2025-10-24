//! Retry logic for NATS operations.

use std::time::Duration;

use crate::{Error, Result};

/// Configuration for retry behavior on failed operations.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (0 means no retries)
    pub max_attempts: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration.
    pub fn new(max_attempts: u32, initial_backoff: Duration) -> Self {
        Self {
            max_attempts,
            initial_backoff,
            max_backoff: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }

    /// Create a configuration with no retries.
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 0,
            initial_backoff: Duration::from_secs(0),
            max_backoff: Duration::from_secs(0),
            backoff_multiplier: 1.0,
        }
    }

    /// Create a configuration with aggressive retries.
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 5,
            initial_backoff: Duration::from_millis(50),
            max_backoff: Duration::from_secs(2),
            backoff_multiplier: 1.5,
        }
    }

    /// Set the maximum backoff duration.
    pub fn with_max_backoff(mut self, max_backoff: Duration) -> Self {
        self.max_backoff = max_backoff;
        self
    }

    /// Set the backoff multiplier.
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Calculate the backoff duration for a given attempt number.
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let backoff_millis = (self.initial_backoff.as_millis() as f64)
            * self.backoff_multiplier.powi(attempt as i32);
        let backoff = Duration::from_millis(backoff_millis as u64);
        backoff.min(self.max_backoff)
    }

    /// Retry an async operation according to this configuration.
    ///
    /// # Example
    /// ```ignore
    /// let config = RetryConfig::default();
    /// let result = config.retry(|| async {
    ///     nats_client.publish("subject", payload).await
    /// }).await?;
    /// ```
    pub async fn retry<F, Fut, T>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.max_attempts {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    // Check if error is retryable
                    if !err.is_retryable() {
                        tracing::debug!(
                            target: crate::TRACING_TARGET_CONNECTION,
                            error = %err,
                            "Non-retryable error, failing immediately"
                        );
                        return Err(err);
                    }

                    last_error = Some(err);

                    // Don't sleep after the last attempt
                    if attempt < self.max_attempts {
                        let backoff = self.calculate_backoff(attempt);
                        tracing::debug!(
                            target: crate::TRACING_TARGET_CONNECTION,
                            attempt = attempt + 1,
                            max_attempts = self.max_attempts,
                            backoff_ms = backoff.as_millis(),
                            "Retrying operation after backoff"
                        );
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        // All retries exhausted
        Err(last_error.unwrap_or_else(|| {
            Error::operation("retry", "All retry attempts exhausted with no error")
        }))
    }

    /// Retry an async operation with a custom retry predicate.
    ///
    /// The predicate determines whether to retry based on the error.
    pub async fn retry_if<F, Fut, T, P>(&self, mut operation: F, mut should_retry: P) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
        P: FnMut(&Error) -> bool,
    {
        let mut last_error = None;

        for attempt in 0..=self.max_attempts {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    // Check custom retry predicate
                    if !should_retry(&err) {
                        tracing::debug!(
                            target: crate::TRACING_TARGET_CONNECTION,
                            error = %err,
                            "Custom predicate rejected retry"
                        );
                        return Err(err);
                    }

                    last_error = Some(err);

                    if attempt < self.max_attempts {
                        let backoff = self.calculate_backoff(attempt);
                        tracing::debug!(
                            target: crate::TRACING_TARGET_CONNECTION,
                            attempt = attempt + 1,
                            max_attempts = self.max_attempts,
                            backoff_ms = backoff.as_millis(),
                            "Retrying operation with custom predicate"
                        );
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            Error::operation("retry_if", "All retry attempts exhausted with no error")
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_backoff, Duration::from_millis(100));
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_no_retry() {
        let config = RetryConfig::no_retry();
        assert_eq!(config.max_attempts, 0);
    }

    #[test]
    fn test_backoff_calculation() {
        let config = RetryConfig::default();

        let backoff0 = config.calculate_backoff(0);
        assert_eq!(backoff0, Duration::from_millis(100));

        let backoff1 = config.calculate_backoff(1);
        assert_eq!(backoff1, Duration::from_millis(200));

        let backoff2 = config.calculate_backoff(2);
        assert_eq!(backoff2, Duration::from_millis(400));
    }

    #[test]
    fn test_max_backoff() {
        let config = RetryConfig::default().with_max_backoff(Duration::from_millis(300));

        let backoff2 = config.calculate_backoff(2);
        assert_eq!(backoff2, Duration::from_millis(300)); // Capped at max
    }

    #[test]
    fn test_builder_methods() {
        let config = RetryConfig::new(5, Duration::from_millis(50))
            .with_max_backoff(Duration::from_secs(1))
            .with_multiplier(3.0);

        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_backoff, Duration::from_millis(50));
        assert_eq!(config.max_backoff, Duration::from_secs(1));
        assert_eq!(config.backoff_multiplier, 3.0);
    }

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let config = RetryConfig::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = config
            .retry(|| {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Error>(42)
                }
            })
            .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_retries() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let config = RetryConfig::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = config
            .retry(|| {
                let count = call_count_clone.clone();
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                    if current < 3 {
                        Err(Error::Timeout {
                            timeout: Duration::from_secs(1),
                        })
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let config = RetryConfig::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = config
            .retry(|| {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, _>(Error::Serialization(serde_json::Error::io(
                        std::io::Error::other("test"),
                    )))
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // Should not retry serialization errors
    }
}
