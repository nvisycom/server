//! Key-value bucket configuration traits.

use std::time::Duration;

/// Marker trait for KV bucket configuration.
///
/// This trait defines the configuration for a NATS KV bucket,
/// similar to `ObjectBucket` for object stores.
pub trait KvBucket: Clone + Send + Sync + 'static {
    /// Bucket name used in NATS KV.
    const NAME: &'static str;

    /// Human-readable description for the bucket.
    const DESCRIPTION: &'static str;

    /// Default TTL for entries in this bucket.
    /// Returns `None` for buckets where entries should not expire.
    const TTL: Option<Duration>;
}

/// Bucket for API authentication tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ApiTokensBucket;

impl KvBucket for ApiTokensBucket {
    const NAME: &'static str = "api_tokens";
    const DESCRIPTION: &'static str = "API authentication tokens";
    const TTL: Option<Duration> = Some(Duration::from_secs(24 * 60 * 60)); // 24 hours
}

/// Bucket for ephemeral chat history sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChatHistoryBucket;

impl KvBucket for ChatHistoryBucket {
    const NAME: &'static str = "chat_history";
    const DESCRIPTION: &'static str = "Ephemeral chat sessions";
    const TTL: Option<Duration> = Some(Duration::from_secs(30 * 60)); // 30 minutes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_tokens_bucket() {
        assert_eq!(ApiTokensBucket::NAME, "api_tokens");
        assert_eq!(
            ApiTokensBucket::TTL,
            Some(Duration::from_secs(24 * 60 * 60))
        );
    }

    #[test]
    fn test_chat_history_bucket() {
        assert_eq!(ChatHistoryBucket::NAME, "chat_history");
        assert_eq!(ChatHistoryBucket::TTL, Some(Duration::from_secs(30 * 60)));
    }
}
