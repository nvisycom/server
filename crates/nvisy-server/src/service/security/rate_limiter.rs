//! In-memory rate limiter implementation using token bucket algorithm.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::handler::{ErrorKind, Result as HandlerResult};

/// Logging target for rate limiter operations
const RATE_LIMITER_TARGET: &str = "nvisy::service::security::rate_limiter";

/// Rate limiter key type
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum RateLimitKey {
    /// Rate limit by IP address
    IpAddress(IpAddr),
    /// Rate limit by account ID
    AccountId(String),
    /// Rate limit by email address
    Email(String),
    /// Custom key
    Custom(String),
}

impl RateLimitKey {
    /// Creates a key from an IP address
    pub fn from_ip(ip: IpAddr) -> Self {
        Self::IpAddress(ip)
    }

    /// Creates a key from an account ID
    pub fn from_account_id(id: impl Into<String>) -> Self {
        Self::AccountId(id.into())
    }

    /// Creates a key from an email address
    pub fn from_email(email: impl Into<String>) -> Self {
        Self::Email(email.into())
    }

    /// Creates a custom key
    pub fn custom(key: impl Into<String>) -> Self {
        Self::Custom(key.into())
    }
}

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Number of tokens available
    tokens: f64,
    /// Maximum number of tokens
    capacity: u32,
    /// Token refill rate per second
    refill_rate: f64,
    /// Last refill time
    last_refill: Instant,
}

impl TokenBucket {
    /// Creates a new token bucket
    fn new(capacity: u32, refill_rate: f64) -> Self {
        Self {
            tokens: capacity as f64,
            capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Refills tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.capacity as f64);
        self.last_refill = now;
    }

    /// Attempts to consume tokens
    fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();

        if self.tokens >= tokens as f64 {
            self.tokens -= tokens as f64;
            true
        } else {
            false
        }
    }

    /// Returns time until next token is available
    fn time_until_available(&self) -> Duration {
        if self.tokens >= 1.0 {
            Duration::from_secs(0)
        } else {
            let tokens_needed = 1.0 - self.tokens;
            let seconds = tokens_needed / self.refill_rate;
            Duration::from_secs_f64(seconds.ceil())
        }
    }
}

/// Rate limiter configuration
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    /// Maximum number of requests allowed in the time window
    pub capacity: u32,
    /// Token refill rate per second
    pub refill_rate: f64,
}

impl RateLimitConfig {
    /// Creates a new rate limit configuration
    pub fn new(capacity: u32, refill_rate: f64) -> Self {
        Self {
            capacity,
            refill_rate,
        }
    }

    /// Creates a configuration for N requests per minute
    pub fn per_minute(requests: u32) -> Self {
        Self {
            capacity: requests,
            refill_rate: requests as f64 / 60.0,
        }
    }

    /// Creates a configuration for N requests per hour
    pub fn per_hour(requests: u32) -> Self {
        Self {
            capacity: requests,
            refill_rate: requests as f64 / 3600.0,
        }
    }

    /// Strict rate limit: 5 requests per minute
    pub fn strict() -> Self {
        Self::per_minute(5)
    }

    /// Moderate rate limit: 20 requests per minute
    pub fn moderate() -> Self {
        Self::per_minute(20)
    }

    /// Lenient rate limit: 60 requests per minute
    pub fn lenient() -> Self {
        Self::per_minute(60)
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self::moderate()
    }
}

/// In-memory rate limiter using token bucket algorithm
#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<RateLimitKey, TokenBucket>>>,
    config: RateLimitConfig,
    cleanup_interval: Duration,
}

impl RateLimiter {
    /// Creates a new rate limiter with the given configuration
    pub fn new(config: RateLimitConfig) -> Self {
        let limiter = Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
            cleanup_interval: Duration::from_secs(300), // 5 minutes
        };

        // Start cleanup task
        limiter.start_cleanup_task();

        tracing::info!(
            target: RATE_LIMITER_TARGET,
            capacity = config.capacity,
            refill_rate = config.refill_rate,
            "Rate limiter initialized"
        );

        limiter
    }

    /// Checks if a request is allowed for the given key
    pub async fn check(&self, key: RateLimitKey) -> HandlerResult<()> {
        self.check_with_cost(key, 1).await
    }

    /// Checks if a request with custom token cost is allowed
    pub async fn check_with_cost(&self, key: RateLimitKey, cost: u32) -> HandlerResult<()> {
        let mut buckets = self.buckets.write().await;

        let bucket = buckets
            .entry(key.clone())
            .or_insert_with(|| TokenBucket::new(self.config.capacity, self.config.refill_rate));

        if bucket.try_consume(cost) {
            Ok(())
        } else {
            let retry_after = bucket.time_until_available();
            tracing::warn!(
                target: RATE_LIMITER_TARGET,
                key = ?key,
                retry_after_secs = retry_after.as_secs(),
                "Rate limit exceeded"
            );
            Err(ErrorKind::TooManyRequests.with_context(format!(
                "Rate limit exceeded. Please try again in {} seconds",
                retry_after.as_secs()
            )))
        }
    }

    /// Resets the rate limit for a specific key
    pub async fn reset(&self, key: &RateLimitKey) {
        let mut buckets = self.buckets.write().await;
        buckets.remove(key);
        tracing::debug!(
            target: RATE_LIMITER_TARGET,
            key = ?key,
            "Rate limit reset"
        );
    }

    /// Clears all rate limit data
    pub async fn clear(&self) {
        let mut buckets = self.buckets.write().await;
        buckets.clear();
        tracing::info!(
            target: RATE_LIMITER_TARGET,
            "All rate limits cleared"
        );
    }

    /// Returns the number of tracked keys
    pub async fn size(&self) -> usize {
        let buckets = self.buckets.read().await;
        buckets.len()
    }

    /// Starts a background task to clean up expired buckets
    fn start_cleanup_task(&self) {
        let buckets = Arc::clone(&self.buckets);
        let interval = self.cleanup_interval;

        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(interval);
            loop {
                cleanup_interval.tick().await;

                let mut buckets = buckets.write().await;
                let before_count = buckets.len();

                // Remove buckets that are at full capacity (inactive)
                buckets.retain(|_, bucket| bucket.tokens < bucket.capacity as f64);

                let removed = before_count - buckets.len();
                if removed > 0 {
                    tracing::debug!(
                        target: RATE_LIMITER_TARGET,
                        removed_count = removed,
                        remaining_count = buckets.len(),
                        "Cleaned up inactive rate limit buckets"
                    );
                }
            }
        });
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("config", &self.config)
            .field("cleanup_interval", &self.cleanup_interval)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_refills_over_time() -> anyhow::Result<()> {
        let config = RateLimitConfig::new(2, 10.0); // 10 tokens per second
        let limiter = RateLimiter::new(config);
        let key = RateLimitKey::from_ip("127.0.0.1".parse()?);

        // Consume all tokens
        assert!(limiter.check(key.clone()).await.is_ok());
        assert!(limiter.check(key.clone()).await.is_ok());
        assert!(limiter.check(key.clone()).await.is_err());

        // Wait for refill
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should have refilled ~2 tokens
        assert!(limiter.check(key.clone()).await.is_ok());

        Ok(())
    }
}
