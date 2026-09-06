//! The concrete event-stream markers, each pinning an [`EventStream`]'s config.

use std::marker::PhantomData;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::core::EventStream;

/// Stream for webhook delivery.
///
/// Messages expire after 1 day. NATS redelivery is the primary retry layer for
/// self-hosted delivery; `ACK_WAIT` exceeds the per-attempt delivery timeout so
/// a slow-but-healthy endpoint is not redelivered while the first attempt is
/// still in flight, and spaces the (at most `MAX_DELIVER`) attempts ~90s apart.
///
/// The payload is left to the consumer (the job type it enqueues), so the stream
/// is generic over `M`; a consumer pins it with a type alias, e.g.
/// `type WebhookStream = nvisy_nats::stream::WebhookStream<WebhookJob>`.
pub enum WebhookStream<M> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> M>),
}

impl<M> EventStream for WebhookStream<M>
where
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Message = M;

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
///
/// Generic over its payload `M`; a consumer pins it with a type alias, e.g.
/// `type ConnectionSyncStream = nvisy_nats::stream::ConnectionSyncStream<SyncJob>`.
pub enum ConnectionSyncStream<M> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> M>),
}

impl<M> EventStream for ConnectionSyncStream<M>
where
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Message = M;

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
///
/// Generic over its payload `M`; a consumer pins it with a type alias, e.g.
/// `type DetectionStream = nvisy_nats::stream::DetectionStream<DetectionJob>`.
pub enum DetectionStream<M> {
    #[doc(hidden)]
    Never(PhantomData<fn() -> M>),
}

impl<M> EventStream for DetectionStream<M>
where
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    type Message = M;

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

    // Consts are payload-independent; any payload pins the generic for the test.
    type Stream = WebhookStream<()>;

    #[test]
    fn test_webhook_stream() {
        assert_eq!(Stream::NAME, "WEBHOOKS");
        assert_eq!(Stream::SUBJECT, "webhooks");
        assert_eq!(Stream::MAX_AGE, Some(Duration::from_secs(24 * 60 * 60)));
        assert_eq!(Stream::CONSUMER_NAME, "webhook-worker");
    }
}
