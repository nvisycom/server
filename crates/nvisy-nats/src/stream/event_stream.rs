//! Event stream configuration for NATS JetStream.

use std::time::Duration;

/// Marker trait for event streams.
///
/// This trait defines the configuration for a NATS JetStream stream.
pub trait EventStream: Clone + Send + Sync + 'static {
    /// Stream name used in NATS JetStream.
    const NAME: &'static str;

    /// Subject pattern for publishing/subscribing to this stream.
    const SUBJECT: &'static str;

    /// Maximum age for messages in this stream.
    /// Returns `None` for streams where messages should not expire.
    const MAX_AGE: Option<Duration>;

    /// Default consumer name for this stream.
    const CONSUMER_NAME: &'static str;
}

/// Stream for webhook delivery.
///
/// Messages expire after 1 day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WebhookStream;

impl EventStream for WebhookStream {
    const CONSUMER_NAME: &'static str = "webhook-worker";
    const MAX_AGE: Option<Duration> = Some(Duration::from_secs(24 * 60 * 60));
    const NAME: &'static str = "WEBHOOKS";
    const SUBJECT: &'static str = "webhooks";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_stream() {
        assert_eq!(WebhookStream::NAME, "WEBHOOKS");
        assert_eq!(WebhookStream::SUBJECT, "webhooks");
        assert_eq!(
            WebhookStream::MAX_AGE,
            Some(Duration::from_secs(24 * 60 * 60))
        );
        assert_eq!(WebhookStream::CONSUMER_NAME, "webhook-worker");
    }
}
