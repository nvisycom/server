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

    /// How long the server waits for an ack before redelivering a message.
    /// `None` uses the JetStream default (30s). Set this above the longest
    /// expected processing time so a slow-but-healthy job is not redelivered
    /// and run a second time concurrently.
    const ACK_WAIT: Option<Duration> = None;

    /// Maximum number of delivery attempts before the server stops redelivering a
    /// message. `None` means unlimited (redeliver until the message ages out).
    /// Set this to bound retries for consumers that nack on failure, so a
    /// permanently-failing message is not redelivered indefinitely.
    const MAX_DELIVER: Option<i64> = None;
}

/// Stream for webhook delivery.
///
/// Messages expire after 1 day. NATS redelivery is the primary retry layer for
/// self-hosted delivery; `ACK_WAIT` exceeds the per-attempt delivery timeout so
/// a slow-but-healthy endpoint is not redelivered while the first attempt is
/// still in flight, and spaces the (at most `MAX_DELIVER`) attempts ~90s apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WebhookStream;

impl EventStream for WebhookStream {
    const ACK_WAIT: Option<Duration> = Some(Duration::from_secs(90));
    const CONSUMER_NAME: &'static str = "webhook-worker";
    const MAX_AGE: Option<Duration> = Some(Duration::from_secs(24 * 60 * 60));
    const MAX_DELIVER: Option<i64> = Some(3);
    const NAME: &'static str = "WEBHOOKS";
    const SUBJECT: &'static str = "webhooks";
}

/// Work queue for connection sync jobs.
///
/// Scheduled syncs are enqueued here. A single shared durable consumer delivers
/// each job to one instance at a time (at-least-once); consumers make jobs
/// idempotent so a redelivery is safe. Messages expire after 1 hour so a
/// backlog cannot pile up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ConnectionSyncStream;

impl EventStream for ConnectionSyncStream {
    // A sync transfer is bounded by a 30-minute timeout; allow ack time to
    // exceed that so a slow-but-healthy job is not redelivered mid-run.
    const ACK_WAIT: Option<Duration> = Some(Duration::from_secs(35 * 60));
    const CONSUMER_NAME: &'static str = "connection-sync-worker";
    const MAX_AGE: Option<Duration> = Some(Duration::from_secs(60 * 60));
    const NAME: &'static str = "CONNECTION_SYNCS";
    const SUBJECT: &'static str = "connection.sync.jobs";
}

/// Work queue for pipeline detection jobs.
///
/// A pipeline run is created synchronously, then its detection (analyze) is
/// enqueued here and handled by a background worker. A single shared durable
/// consumer delivers each job to one instance at a time (at-least-once); the
/// worker is idempotent on the run so a redelivery is safe. Messages expire
/// after 1 hour so a backlog cannot pile up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DetectionStream;

impl EventStream for DetectionStream {
    // Detection can be slow (LLM/OCR); allow ack time to exceed the longest
    // expected analyze so a slow-but-healthy job is not redelivered mid-run.
    const ACK_WAIT: Option<Duration> = Some(Duration::from_secs(15 * 60));
    const CONSUMER_NAME: &'static str = "detection-worker";
    const MAX_AGE: Option<Duration> = Some(Duration::from_secs(60 * 60));
    const NAME: &'static str = "DETECTIONS";
    const SUBJECT: &'static str = "pipeline.detection.jobs";
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
